extern crate rand;
extern crate clap;
extern crate termion;
extern crate chrono;
extern crate vocage;


use termion::event::Key;
use termion::screen::AlternateScreen;
use termion::input::TermRead;
use termion::raw::{IntoRawMode};
use termion::color;
use std::io::{Write, stdout, stdin};
use clap::{Arg, App};
use rand::prelude::{thread_rng,Rng};
use vocage::{VocaSession,VocaCard,PrintFormat,load_files};

static NUMCHARS: &[char] = &['1','2','3','4','5','6','7','8','9'];

fn main() {
    let args = App::new("Vocage :: Flash cards")
                  .version("1.0")
                  .author("Maarten van Gompel (proycon) <proycon@anaproy.nl>")
                  .about("A simple command-line flash card system implementing spaced-repetition (Leitner)")
                  .arg(Arg::with_name("force")
                    .long("force")
                    .short("-f")
                    .help("when loading multiple files, force the metadata of the first one on all the others")
                   )
                  .arg(Arg::with_name("all")
                    .long("all")
                    .short("-a")
                    .help("Consider all cards, not only the ones that are due. Can also be toggled at runtime with 'a'")
                   )
                  .arg(Arg::with_name("limit")
                    .long("limit")
                    .short("-L")
                    .takes_value(true)
                    .help("Limit to this deck only (all decks will be considered by default)")
                   )
                  .arg(Arg::with_name("firstdeck")
                    .long("firstdeck")
                    .short("-A")
                    .takes_value(true)
                    .help("Limit decks, set this as first deck (number), and ignore lower decks")
                   )
                  .arg(Arg::with_name("lastdeck")
                    .long("lastdeck")
                    .short("-Z")
                    .takes_value(true)
                    .help("Limit decks, set this as last deck (number), and ignore higher decks")
                   )
                  .arg(Arg::with_name("files")
                    .help("vocabulary file (tsv)")
                    .takes_value(true)
                    .multiple(true)
                    .index(1)
                    .required(true)
                   )
                  .args(&VocaSession::common_arguments())
                  .arg(Arg::with_name("minimal")
                    .takes_value(true)
                    .short("-m")
                    .long("minimal")
                    .help("Minimal interface, no TUI, just print to stdout. The value for this parameter is either 'plain' or 'colour', the latter of which will still produce ANSI colours.")
                   )
                  .arg(Arg::with_name("seen")
                    .long("seen")
                    .short("-s")
                    .help("Only present cards that have been previously seen (no unseen cards). Can also be toggled at runtime with 's'")
                   )
                  .arg(Arg::with_name("ordered")
                    .long("ordered")
                    .short("-z")
                    .help("Show cards in the order they are defined rather than randomly. Can also be toggled at runtime with 'z'")
                   )
                  .arg(Arg::with_name("reset")
                    .long("reset")
                    .help("Reset the loaded deck, this strips the due date and deck assignment of all cards")
                   )
                  .get_matches();


    let mut rng = thread_rng();

    let mut datasets = load_files(args.values_of("files").unwrap().collect(), args.is_present("force"), args.is_present("reset"));
    for dataset in datasets.iter_mut() {
        dataset.session.set_common_arguments(&args).expect("setting common arguments");
        if dataset.session.decks.is_empty() && dataset.session.intervals.is_empty() {
            //no decks or intervals defined yet, set some defaults
            dataset.session.decks = vec!("immediate","daily","weekly","monthly","quarterly","yearly").iter().map(|s| s.to_string()).collect();
            dataset.session.intervals = vec!(0,1440,10080,43200,129600,518400);
        }
    }

    let limit_decks: Option<Vec<u8>> = if args.is_present("limit") {
        Some(vec!(datasets[0].session.get_deck_by_name(args.value_of("limit").unwrap()).unwrap()))
    } else if args.is_present("firstdeck") || args.is_present("lastdeck") {
        let firstdeck: u8 = args.value_of("firstdeck").map(|s| s.parse::<u8>().expect("expecting an integer")  - 1).unwrap_or(0);
        let lastdeck: u8 = args.value_of("lastdeck").map(|s| s.parse::<u8>().expect("expecting an integer") ).unwrap_or(255);
        Some((firstdeck..lastdeck).collect())
    } else {
        None
    };
    let mut due_only: bool = !args.is_present("all");
    let mut seen_only: bool = args.is_present("seen");
    let mut ordered: bool = args.is_present("ordered");
    let mut reset: bool = args.is_present("reset");
    let minimal: Option<PrintFormat> = match args.value_of("minimal") {
        None => None,
        Some("color") | Some("colour") => Some(PrintFormat::AnsiColour),
        _ => Some(PrintFormat::Plain),
    };

    let mut done = false;

    let mut stdout: Box<dyn Write> = if minimal.is_none() {
        Box::new(stdout().into_raw_mode().unwrap())
    } else {
        Box::new(stdout())
    };

    let mut status: String = "Use: q to quit, w to save, space to flip, l/→ to promote, h/← to demote, j/↓ for next".to_owned();

    let mut history: Vec<(usize,usize)> = Vec::new();
    let mut pick_specific: Option<(usize,usize)> = None; //(set,card), will select a random card if set to None

    let mut duecards = 0;
    let mut tries = 0;
    let mut changed = false;
    let mut confirmexitstage = false;
    let mut pressed_q = false;

    //make a copy to prevent problems with the borrow checker
    let session = datasets[0].session.clone();

    while !done {
        if changed {
            reset = false;
        }
        if let Some(card) = match pick_specific {
                Some((setindex, cardindex)) => datasets[setindex].cards.get_mut(cardindex), //pick a specific card
                None => {
                    if ordered {
                        //pick a card in order
                        let (setindex, cardindex) = history.last().unwrap_or(&(0,0));
                        let mut nextindex = None;
                        let mut cardindex = *cardindex;
                        let mut setindex = *setindex;
                        for i in setindex..datasets.len() {
                            nextindex = datasets[i].next_index(cardindex, limit_decks.as_ref(), due_only, seen_only, history.is_empty());
                            if nextindex.is_some() {
                                setindex = i;
                                break;
                            }
                            cardindex = 0;
                        }
                        if let Some((cardindex,totalcards)) = nextindex {
                            duecards = totalcards;
                            history.push((setindex,cardindex));
                            datasets[setindex].cards.get_mut(cardindex)
                        } else {
                            tries = 999; //no indeterministic factor
                            None
                        }
                    } else {
                        //pick a random set
                        let setindex = if datasets.len() == 1 { 0 } else { rng.gen_range(0,datasets.len()) };
                        //pick a random card
                        if let Some((cardindex,totalcards)) = datasets[setindex].random_index(&mut rng, limit_decks.as_ref(), due_only, seen_only) {
                            duecards = totalcards;
                            history.push((setindex,cardindex));
                            tries = 0; //reset
                            datasets[setindex].cards.get_mut(cardindex)
                        } else {
                            tries += 1;
                            None
                        }
                    }
                }
            } { //end match block. In ordered mode, cards will be presented in the order they are defined.
            pick_specific = None; //reset
            //show card
            let mut side: u8 = 0;
            draw(&mut stdout, Some(card), &session, side, status.as_str(), history.len(), duecards, minimal);
            status.clear();

            //process input
            for c in stdin().keys() {
                match c.unwrap() {
                     Key::Char('w') => {
                         for dataset in datasets.iter() {
                             dataset.write(reset).expect("failure saving file");
                         }
                         status = "Saved...".to_owned();
                         pick_specific = history.pop(); //make sure we re-show the current item
                         if pick_specific.is_some() {
                             history.push(pick_specific.clone().unwrap());
                         }
                         changed = false;
                         if confirmexitstage {
                             done = true;
                         } else {
                             confirmexitstage = false;
                         }
                         break;
                     },
                     Key::Char('Q') => {
                         done = true;
                         break;
                     },
                     Key::Char('q') | Key::Esc => {
                         pressed_q = true;
                         if !changed || confirmexitstage {
                             done = true;
                             break;
                         } else {
                             confirmexitstage = true;
                             status = "Changes have not been saved yet! Press w to save or q again to force quit".to_owned();
                             break;
                         }
                     },
                     Key::Char(' ') | Key::Char('\n') => {
                         side += 1;
                         if side >= session.showcolumns.len() as u8 {
                             side = 0;
                         }
                         //redraw
                         draw(&mut stdout, Some(card), &session, side, status.as_str(), history.len(), duecards, minimal);
                     },
                     Key::Char('h') | Key::Left => {
                         if card.demote(&session) {
                             status = format!("Card demoted to deck {}: {}", card.deck+1, session.decks.get(card.deck as usize).unwrap_or(&"unspecified".to_owned())  ).to_owned();
                             changed = true;
                         } else {
                             status = "Already on first deck".to_owned();
                         }
                         break;
                     },
                     Key::Char('l') | Key::Right => {
                         if card.promote(&session) {
                             status = format!("Card promoted to deck {}: {}", card.deck+1, session.decks.get(card.deck as usize).unwrap_or(&"unspecified".to_owned())  ).to_owned();
                             changed = true;
                         } else {
                             status = "Already on last deck".to_owned();
                         }
                         break;
                     },
                     Key::Char('j') | Key::Down => {
                         card.move_to_deck(card.deck, &session);
                         status = format!("Card retained on deck {}: {}", card.deck+1, session.decks.get(card.deck as usize).unwrap_or(&"unspecified".to_owned())  ).to_owned();
                         changed = true;
                         break;
                     },
                     Key::Char('J') | Key::PageDown => {
                         status = "Card skipped".to_owned();
                         break;
                     },
                     Key::Char('k') | Key::Up | Key::PageUp => {
                         status = "Showing previous card".to_owned();
                         pick_specific = history.pop();
                         break;
                     },
                     Key::Char(c) if NUMCHARS.contains(&c) => {
                         let targetdeck = c as u8 - 49;
                         if card.move_to_deck(targetdeck, &session) {
                             status = format!("Card moved to deck {}: {}", card.deck+1, session.decks.get(card.deck as usize).unwrap_or(&"unspecified".to_owned())  ).to_owned();
                         } else {
                             status = "Invalid deck".to_owned();
                         }
                         break;
                     },
                     Key::Char('s') => {
                         seen_only = !seen_only;
                         if seen_only {
                             status = "Only showing cards that have been seen already".to_owned();
                         } else {
                             status = "Previously unseen cards will be presented again".to_owned();
                         }
                     },
                     Key::Char('a') => {
                         due_only = !due_only;
                         if due_only {
                             status = "Only showing cards that are due".to_owned();
                         } else {
                             status = "Showing all cards, including those not due".to_owned();
                         }
                     },
                     Key::Char('z') => {
                         ordered = !ordered;
                         if ordered {
                             status = "Presenting cards in predefined order".to_owned();
                         } else {
                             status = "Presenting cards in random order".to_owned();
                         }
                     },
                     _ => {
                         status = "Key not bound".to_owned();
                     }
                };
                if !pressed_q {
                     confirmexitstage = false; //reset
                }
            }
        } else if tries > 100 { //after a hundred attempted picks we give up
            write!(stdout, "{}{}{}{}",
                   termion::clear::All,
                   termion::cursor::Goto(1, 5),
                   "No more cards are due for now, well done! Saving and exiting...",
                   termion::cursor::Hide).expect("error drawing");

             for dataset in datasets.iter() {
                 dataset.write(reset).expect("failure saving file");
             }
             done = true;
        }
    }
    write!(stdout,"{}\n",termion::cursor::Show).expect("error drawing");
}


pub fn draw(stdout: &mut impl Write, card: Option<&VocaCard>, session: &VocaSession, side: u8, status: &str, seqnr: usize, duecards: usize, minimal: Option<PrintFormat>) {

    let mut stdout = AlternateScreen::from(stdout);

    let (width, height) = if minimal.is_none() {
        termion::terminal_size().expect("terminal size")
    } else {
        (0,0)
    };

    if minimal.is_none() && (width < 15 || height < 6) {
        write!(stdout, "{}{}Terminal too small!{}",
               termion::clear::All,
               termion::cursor::Goto(1, 1),
               termion::cursor::Hide).expect("error drawing");
        return;
    }

    if minimal.is_none() {
        write!(stdout, "{}{}{}{}",
               termion::clear::All,
               termion::cursor::Goto(1, 1),
               status,
               termion::cursor::Hide).expect("error drawing");
    }

    if let Some(card) = card {
        let lines = card.fields_to_str(side, &session, true).expect("printing card failed (no such side?)");
        let halftextheight: u16 = (lines.len() / 2) as u16;
        let y = 1 + if height == 0 {
            0 //just so we dont fail in minimal mode
        } else if height / 2 > halftextheight {
            height / 2 - halftextheight
        } else {
            1
        };
        for (i, (column, line)) in lines.into_iter().enumerate() {
            let halftextwidth: u16 = (line.chars().count() / 2) as u16;
            let x = if width == 0 {
                0 //just so we dont fail in minimal mode
            } else if width / 2 > halftextwidth {
                width / 2 - halftextwidth
            } else {
                1
            };
            let c: color::Fg<&dyn color::Color> =
                   match column {
                        0 => color::Fg(&color::Green),
                        1 => color::Fg(&color::Cyan),
                        2 => color::Fg(&color::Yellow),
                        3 => color::Fg(&color::Magenta),
                        4 => color::Fg(&color::Blue),
                        _ => color::Fg(&color::Reset),
                   };
            if let Some(minimal) = minimal {
                if minimal == PrintFormat::AnsiColour {
                    write!(stdout,"{}{}{}\n",c,line,termion::color::Fg(color::Reset)).expect("error drawing (minimal)");
                } else {
                    write!(stdout,"{}\n",line).expect("error drawing (minimal)");
                }
            } else {
                write!(stdout,"{}{}{}{}{}",
                   termion::cursor::Goto(x, y + i as u16),
                   c,
                   line,
                   termion::color::Fg(color::Reset),
                   termion::cursor::Hide).expect("error drawing");
            }
        }
        if minimal.is_none() {
            write!(stdout,"{}{}{}",
               termion::cursor::Goto(1,height),
               format!("#{}/{} - Deck: {} ({}/{}) - Due: {} ({})",
                    seqnr,
                    duecards,
                    session.decks.get(card.deck as usize).unwrap_or(&"none".to_owned()),
                    card.deck+1,
                    session.decks.len(),
                    match card.due {
                        Some(datetime) => datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
                        None => "any time".to_owned()
                    },
                    match session.intervals.get(card.deck as usize) {
                        Some(i) if *i >= 1440 => {
                            format!("{} days",i/1440)
                        },
                        Some(i) if *i >= 60 => {
                            format!("{} hours",i/60)
                        },
                        Some(i) => {
                            format!("{} mins",i)
                        },
                        None => {
                            "immediate".to_owned()
                        }
                    },
               ),
               termion::cursor::Hide).expect("error drawing");
        } else {
            write!(stdout,"\n").expect("error writing");
        }
    }

    stdout.flush().unwrap();
}
