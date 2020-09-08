extern crate rand;
extern crate clap;
extern crate termion;
extern crate chrono;
extern crate vocage;


use termion::event::Key;
use termion::input::TermRead;
use termion::raw::{IntoRawMode,RawTerminal};
use termion::color;
use std::io::{Write, stdout, stdin, Stdout};
use clap::{Arg, App};
use rand::prelude::{thread_rng,Rng};
use vocage::{VocaSession,VocaCard,load_files};

static NUMCHARS: &[char] = &['1','2','3','4','5','6','7','8','9'];

fn main() {
    let args = App::new("Vocage :: Flash cards")
                  .version("0.1")
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
                    .help("Consider all cards, not only the ones that are due")
                   )
                  .arg(Arg::with_name("limit")
                    .long("limit")
                    .short("-L")
                    .takes_value(true)
                    .help("Limit to this deck only (all decks will be considered by default)")
                   )
                  .arg(Arg::with_name("files")
                    .help("vocabulary file (tsv)")
                    .takes_value(true)
                    .multiple(true)
                    .index(1)
                    .required(true)
                   )
                  .args(&VocaSession::common_arguments())
                  .get_matches();


    let mut rng = thread_rng();

    let mut datasets = load_files(args.values_of("files").unwrap().collect(), args.is_present("force"));
    for dataset in datasets.iter_mut() {
        dataset.session.set_common_arguments(&args).expect("setting common arguments");
        if dataset.session.decks.is_empty() && dataset.session.intervals.is_empty() {
            //no decks or intervals defined yet, set some defaults
            dataset.session.decks = vec!("immediate","daily","weekly","monthly","quarterly","yearly").iter().map(|s| s.to_string()).collect();
            dataset.session.intervals = vec!(0,1440,10080,43200,129600,518400);
        }
    }

    let deck: Option<u8> = if args.is_present("limit") {
        Some(datasets[0].session.get_deck_by_name(args.value_of("limit").unwrap()).unwrap())
    } else {
        None
    };
    let due_only: bool = !args.is_present("all");

    let mut done = false;

    let mut stdout = stdout().into_raw_mode().unwrap();

    let mut status: String = "Use: q to quit, w to save, space to flip, l/→ to promote, h/← to demote, j/↓ for next".to_owned();

    let mut history: Vec<(usize,usize)> = Vec::new();
    let mut pick_specific: Option<(usize,usize)> = None; //(set,card), will select a random card if set to None

    let mut duecards = 0;
    let mut tries = 0;

    //make a copy to prevent problems with the borrow checker
    let session = datasets[0].session.clone();

    while !done {
        if let Some(card) = match pick_specific {
                Some((setindex, cardindex)) => datasets[setindex].cards.get_mut(cardindex),
                None => {
                    //pick a random set
                    let setindex = if datasets.len() == 1 { 0 } else { rng.gen_range(0,datasets.len()) };
                    //pick a random card
                    if let Some((cardindex,totalcards)) = datasets[setindex].random_index(&mut rng, deck, due_only) {
                        duecards = totalcards;
                        history.push((setindex,cardindex));
                        tries = 0; //reset
                        datasets[setindex].cards.get_mut(cardindex)
                    } else {
                        tries += 1;
                        None
                    }
                }
            } {
            pick_specific = None; //reset
            //show card
            let mut side: u8 = 0;
            draw(&mut stdout, Some(card), &session, side, status.as_str(), history.len(), duecards);
            status.clear();

            //process input
            for c in stdin().keys() {
                match c.unwrap() {
                     Key::Char('w') => {
                         for dataset in datasets.iter() {
                             dataset.write().expect("failure saving file");
                         }
                         status = "Saved...".to_owned();
                         pick_specific = history.pop(); //make sure we re-show the current item
                         if pick_specific.is_some() {
                             history.push(pick_specific.clone().unwrap());
                         }
                         break;
                     },
                     Key::Char('q') | Key::Esc => {
                         done = true;
                         break;
                     },
                     Key::Char(' ') | Key::Char('\n') => {
                         side += 1;
                         if side >= session.showcolumns.len() as u8 {
                             side = 0;
                         }
                         //redraw
                         draw(&mut stdout, Some(card), &session, side, status.as_str(), history.len(), duecards);
                     },
                     Key::Char('h') | Key::Left => {
                         if card.demote(&session) {
                             status = format!("Card demoted to deck {}", card.deck+1).to_owned();
                         } else {
                             status = "Already on first deck".to_owned();
                         }
                         break;
                     },
                     Key::Char('l') | Key::Right => {
                         if card.promote(&session) {
                             status = format!("Card promoted to deck {}", card.deck+1).to_owned();
                         } else {
                             status = "Already on last deck".to_owned();
                         }
                         break;
                     },
                     Key::Char('j') | Key::Down => {
                         status = "Card skipped".to_owned();
                         break;
                     },
                     Key::Char('k') | Key::Up => {
                         pick_specific = history.pop();
                         break;
                     },
                     Key::Char(c) if NUMCHARS.contains(&c) => {
                         let targetdeck = c as u8 - 49;
                         if card.move_to_deck(targetdeck, &session) {
                             status = format!("Card promoted to deck {}", targetdeck+1).to_owned();
                         } else {
                             status = "Already on last deck".to_owned();
                         }
                         break;
                     },
                     _ => {
                     }
                };
            }
        } else if tries > 100 { //after a hundred attempted picks we give up
            write!(stdout, "{}{}{}{}",
                   termion::clear::All,
                   termion::cursor::Goto(1, 5),
                   "No more cards are due for now, well done! Saving and exiting...",
                   termion::cursor::Hide).expect("error drawing");

             for dataset in datasets.iter() {
                 dataset.write().expect("failure saving file");
             }
             done = true;
        }
    }
    write!(stdout,"{}\n",termion::cursor::Show).expect("error drawing");
}


pub fn draw(stdout: &mut RawTerminal<Stdout>, card: Option<&VocaCard>, session: &VocaSession, side: u8, status: &str, seqnr: usize, duecards: usize) {
    let (width, height) = termion::terminal_size().expect("terminal size");
    if width < 15 || height < 6 {
        write!(stdout, "{}{}Terminal too small!{}",
               termion::clear::All,
               termion::cursor::Goto(1, 1),
               termion::cursor::Hide).expect("error drawing");
        return;
    }

    write!(stdout, "{}{}{}{}",
           termion::clear::All,
           termion::cursor::Goto(1, 1),
           status,
           termion::cursor::Hide).expect("error drawing");

    if let Some(card) = card {
        let lines = card.fields_to_str(side, &session, true).expect("printing card failed (no such side?)");
        let halftextheight: u16 = (lines.len() / 2) as u16;
        let y = 1 + if height / 2 > halftextheight {
            height / 2 - halftextheight
        } else {
            1
        };
        for (i, (column, line)) in lines.into_iter().enumerate() {
            let halftextwidth: u16 = (line.chars().count() / 2) as u16;
            let x = if width / 2 > halftextwidth {
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
            write!(stdout,"{}{}{}{}{}",
               termion::cursor::Goto(x, y + i as u16),
               c,
               line,
               termion::color::Fg(color::Reset),
               termion::cursor::Hide).expect("error drawing");
        }
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
    }

    stdout.flush().unwrap();
}
