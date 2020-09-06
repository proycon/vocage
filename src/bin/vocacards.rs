extern crate rand;
extern crate clap;
extern crate termion;
extern crate vocage;


use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use std::io::{Write, stdout, stdin};
use clap::{Arg, App};
use rand::prelude::{thread_rng,Rng};
use vocage::{VocaData,VocaCard,getinputline,load_files};


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
                    .short("-l")
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
                  .get_matches();


    let mut rng = thread_rng();

    let mut datasets = load_files(args.values_of("files"), args.is_present("force"));

    let deck: Option<u8> = if args.is_present("limit") {
        Some(datasets[0].session.get_deck_by_name(args.value_of("limit").unwrap()))
    } else {
        None
    };
    let due_only: bool = !args.is_present("all");

    let done = false;

    let stdin = stdin();
    let mut stdout = stdout().into_raw_mode().unwrap();

    let side: u8 = 0;

    let mut status: String = "Use: q to save & exit, space to flip, l/→ to promote, h/← to demote, j/↓ for next".to_owned();


    while !done {
        let setchoice = if datasets.len() == 1 { 0 } else { rng.gen_range(0,datasets.len()) };
        if let Some(card) = datasets[setchoice].pick_card_mut(&mut rng, deck, due_only) {
            //show card
            draw(Some(card), side, status.as_str());

            //process input
            for c in stdin.keys() {
                match c.unwrap() {
                     Key::Char('w') => {
                         for dataset in datasets.iter() {
                             dataset.write();
                         }
                         status = "Saved...";
                         break;
                     },
                     Key::Char('q') | Key::Esc => {
                         for dataset in datasets.iter() {
                             dataset.write();
                         }
                         done = true;
                         break;
                     },
                     Key::Space | Key::Return => {
                         side += 1;
                         if side >= card.fields.len() {
                             side = 0;
                         }
                         break;
                     },
                     Key::Char('h') | Key::Left => {
                         card.demote();
                         status = "Card demoted";
                         break;
                     },
                     Key::Char('l') | Key::Right => {
                         card.promote();
                         status = "Card promoted";
                         break;
                     },
                };
            }
        }
    }
}


pub fn draw(stdout: &mut SOMETHING, card: Option<&VocaCard>, side: u8, status: &str) {
    write!(stdout, "{}{}{}",
           termion::clear::All,
           termion::cursor::Goto(1, 1),
           status,
           termion::cursor::Hide);

    if let Some(card) = card {
    }

    stdout.flush().unwrap();
}
