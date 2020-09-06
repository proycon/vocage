extern crate rand;
extern crate clap;
extern crate vocage;

use clap::{Arg, App};
use rand::prelude::{thread_rng,Rng};
use vocage::{VocaData,getinputline};

fn main() {
    let args = App::new("Vocage :: Flash cards")
                  .version("0.1")
                  .author("Maarten van Gompel (proycon) <proycon@anaproy.nl>")
                  .about("A simple command-line flash card system implementing spaced-repetition (Leitner)")
                  //snippet hints --> addargb,addargs,addargi,addargf,addargpos
                  .arg(Arg::with_name("filename")
                    .help("vocabulary file (tsv)")
                    .takes_value(true)
                    .index(1)
                    .required(true)
                   )
                  .get_matches();

    //hints: matches.is_present() , matches.value_of()
    //

    let mut rng = thread_rng();

    match VocaData::from_file(args.value_of("filename").unwrap()) {
        Err(err) => eprintln!("ERROR: {}", err),
        Ok(mut data) => {
            loop {
                if let Some(card) = data.pick_card_mut(&mut rng, deck, due_only) {
                    if let Some(response) = getinputline() {
                        handle_response(response, &mut card);
                    }
                }
            }
        }
    }

}

fn handle_response(response: String, card: &mut VocaCard)  {
    let response: Vec<&str> = response.split(" ").collect();
    if !response.is_empty() {
        match response[0] {
            "q" | "exit" | "quit" => {
                data.write().expect("error writing");
                std::process::exit(0);
            },
            "w" | "write" | "save" => {
                data.write().expect("error writing");
            },
            _ => {
                eprintln!("Invalid command");
            }
        }
    }
}
