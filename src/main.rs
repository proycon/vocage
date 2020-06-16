extern crate clap;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate regex;
extern crate ansi_term;
extern crate dirs;
extern crate simple_error;
extern crate chrono;

use std::iter::Iterator;
use std::io::{BufRead,Write};
use std::path::{Path,PathBuf};
use std::process::exit;
use std::fs;
use clap::{App, Arg, SubCommand};
use regex::Regex;
use rand::{thread_rng,Rng};
use ansi_term::Colour::{Red,Green, Blue};
use self::simple_error::SimpleError;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::prelude::*;
use vocage::*;


trait SessionInterface {
    fn prompt(&self) -> Option<String>;
    fn handle_response(&mut self, response: String, datadir: &str, sessiondir: &str) -> bool;
}

impl SessionInterface for VocaSession {
    fn prompt(&self) -> Option<String> {
        print!("Session: {}  ", PathBuf::from(self.filename.as_str()).file_name().unwrap().to_str().unwrap());
        print!("Data: {}  ", PathBuf::from(self.set_filename.as_str()).file_name().unwrap().to_str().unwrap());
        print!("Mode: {}", self.mode);
        if let Some(deck_index) = self.deck_index {
            print!("  Deck: #{}/{} {}", deck_index, self.decks.len(), self.deck_names.get(deck_index).expect("getting name for deck") );
            if let Some(card_index) = self.card_index {
                print!("  Card: #{}/{}", card_index, self.decks[deck_index].len() );
            } else {
                //this is not really a state we should encounter much
                print!("  Card: none/{}", self.decks[deck_index].len() );
            }
            println!("");
        } else {
            print!("  Deck: none");
            println!("");
        }
        getinputline()
    }

    fn handle_response(&mut self, response: String, datadir: &str, sessiondir: &str) -> bool {
        let mut handled = match self.mode {
            _ => false,
        };
        let response: Vec<&str> = response.split(" ").collect();
        if !response.is_empty() {
            handled = match response[0] {
                "set" => {
                    if let Some(key) = response.get(1) {
                        self.set(key.to_string());
                    } else {
                        eprintln!("No setting specified")
                    }
                    true
                },
                "unset" => {
                    if let Some(key) = response.get(1) {
                        self.unset(key);
                    } else {
                        eprintln!("No setting specified")
                    }
                    true
                },
                "toggle" => {
                    if let Some(key) = response.get(1) {
                        if self.toggle(key.to_string()) {
                            eprintln!("enabled")
                        } else {
                            eprintln!("disabled")
                        }
                    } else {
                        eprintln!("No setting specified")
                    }
                    true
                },
                "deck" => {
                    if let Some(deck_value) = response.get(1) {
                        if deck_value.chars().all(|c| c.is_numeric()) {
                            if let Ok(deck_index) = deck_value.parse::<usize>() {
                               self.select_deck(deck_index);
                            }
                        } else {
                           self.select_deck_by_name(deck_value);
                        }
                    } else {
                        eprintln!("Provide a deck name or number")
                    }
                    true
                },
                "nodeck" => {
                    self.unselect_deck();
                    true
                },
                "nextdeck" | "nd"  => {
                    self.next_deck().map_err(|e| eprintln!("{}",e) );
                    true

                }
                "prevdeck" | "pd" => {
                    self.previous_deck().map_err(|e| eprintln!("{}",e) );
                    true
                }
                "decks" => {
                    for (i, deck_name) in self.deck_names.iter().enumerate() {
                        println!("#{}: {}", i+1, deck_name);
                    }
                    true
                },
                "cards" | "ls" => {
                    for (i, card) in self.iter().enumerate() {
                        println!("#{}: {}", i+1, card);
                    }
                    true
                },
                "promote" | "correct" | "+" => {
                    self.promote().map_err(|e| eprintln!("{}",e) );
                    true
                },
                "demote" | "incorrect" | "-" => {
                    self.demote().map_err(|e| eprintln!("{}",e) );
                    true
                },
                "next" | "n"  => {
                    self.next_card().map_err(|e| eprintln!("{}",e) );
                    true
                },
                "previous" | "prev" | "p"  => {
                    self.previous_card().map_err(|e| eprintln!("{}",e) );
                    true
                }
                "shuffle" | "x"  => {
                    self.shuffle().map_err(|e| eprintln!("{}",e) );
                    true
                },
                "mode" => {
                    if let Some(mode_string) = response.get(1) {
                        match VocaMode::from_str(mode_string) {
                            Ok(mode) => self.mode = mode,
                            Err(err) => eprintln!("{}", err)
                        }
                    } else {
                        eprintln!("No mode specified")
                    }
                    true
                },
                "help" => {
                    println!("cards                                  -- Show a list of all cards in the deck (or all cards that exist if no deck is selected)");
                    println!("deck [name|index]                      -- Switch to the deck by name or number");
                    println!("demote                                 -- Demote the current card to the previous deck");
                    println!("mode flashcards                        -- Switch to flashcards mode");
                    println!("mode openquiz                          -- Switch to open quiz mode");
                    println!("mode multiquiz                         -- Switch to multiple-choice quiz mode");
                    println!("next                                   -- Present the next card");
                    println!("nextdeck                               -- Switch to the next deck");
                    println!("nodeck                                 -- Deselect a deck");
                    println!("previous                               -- Present the previous card");
                    println!("prevdeck                               -- Switch to the previous deck");
                    println!("promote                                -- Promote the current card to the next deck");
                    println!("shuffle                                -- Shuffle the deck (randomizing the card order)");
                    println!("----");
                    false //we condider this unhandled so the handling falls back and also output the general commands later on
                }
                _ => false,
            };
        }
        handled
    }
}


fn getinputline() -> Option<String> {
    print!(">>> ");
    std::io::stdout().flush().unwrap();
    let stdin = std::io::stdin();
    if let Some(response) = stdin.lock().lines().next() { //read one line only
        if let Ok(response) = response {
            if response != "" {
                return Some(response);
            }
        }
    }
    None
}

fn handle_response(response: String, mut session: Option<VocaSession>, datadir: &str, sessiondir: &str) -> Option<VocaSession> {
    let response: Vec<&str> = response.split(" ").collect();
    if !response.is_empty() {
        match response[0] {
            "q" | "exit" | "quit" => {
                if let Some(session) = session {
                    match session.save() {
                        Ok(()) => exit(0),
                        Err(err) => {
                            eprintln!("{}",err);
                            exit(1)
                        }
                    }
                }
                exit(0);
            },
            "ls" | "list" | "sets" => {
                let sets: Vec<String> = getdataindex(Some(PathBuf::from(datadir)));
                for s in sets.iter() {
                    println!("{}", s.as_str());
                }
            },
            "ps" | "sessions" => {
                let sessions: Vec<String> = getsessionindex(Some(PathBuf::from(sessiondir)));
                for s in sessions.iter() {
                    println!("{}", s.as_str());
                }
            },
            "resume" => {
                //resume an existing session
                if let Some(filename) = response.get(1) {
                    let filename: String = if PathBuf::from(filename).exists() {
                        filename.to_string()
                    } else {
                        getsessionfile(filename, PathBuf::from(sessiondir)).to_string_lossy().to_string()
                    };
                    if let Ok(loaded_session) = VocaSession::from_file(filename.as_str()) {
                        session = Some(loaded_session);
                    } else {
                        eprintln!("Unable to load session file: {}", filename);
                    }
                } else {
                    eprintln!("No session file specified as first argument");
                }
            },
            "start" => {
                //start a new session
                if let Some(filename) = response.get(1) {
                    let mut session_filename: String = if let Some(session_filename) = response.get(2) {
                        session_filename.to_string()
                    } else {
                        let mut session_filename: String = String::new();
                        session_filename.push_str(PathBuf::from(filename).file_stem().unwrap().to_str().unwrap());
                        session_filename = session_filename.replace(".json","");
                        session_filename = session_filename.replace(".yaml","");
                        session_filename.push_str("-");
                        let dt: DateTime<Local> = Local::now();
                        session_filename.push_str(dt.format("%Y%m%d-%H%M").to_string().as_str());
                        session_filename
                    };
                    if !session_filename.ends_with(".json") {
                        session_filename.push_str(".json");
                    }
                    if !session_filename.starts_with(".") && !session_filename.starts_with("/") {
                        session_filename = getsessionfile(session_filename.as_str(), PathBuf::from(sessiondir)).to_string_lossy().to_string();
                    }
                    let filename = if !filename.starts_with(".") && !filename.starts_with("/") {
                        getdatafile(filename,  PathBuf::from(datadir)).to_string_lossy().to_string()
                    } else {
                        filename.to_string()
                    };
                    let deck_names: Vec<String> = if let Some(deck_names) = response.get(3) {
                        deck_names.split(",").map(|s| s.to_owned()).collect()
                    } else {
                        vec!("new".to_string(),"short".to_string(),"medium".to_string(),"long".to_string(),"done".to_string())
                    };
                    match VocaSession::new(session_filename, filename.to_string(), deck_names) {
                        Ok(new_session) => {
                            session = Some(new_session);
                        },
                        Err(err) => {
                            eprintln!("Unable to start session: {}", err);
                        }
                    }
                }
            },
            "help" => {
                println!("quit                                   -- Save session & quit");
                println!("resume [session]                       -- Load and resume an existing session");
                println!("sets                                   -- List all sets");
                println!("sessions                               -- List all session");
                println!("start [set] [[session]] [[deck_names]] -- Start a new session using the specified set");
            },
            _ => {
                eprintln!("Invalid command");
            }
        };
    }
    session
}

fn main() {
    let mut success = true; //determines the exit code
    let defaultdatadir = defaultdatadir();
    let defaultsessiondir = defaultsessiondir();
    let argmatches = App::new("Vocage")
        .version("0.1")
        .author("Maarten van Gompel (proycon) <proycon@anaproy.nl>")
        .about("Games for learning vocabulary")
        .arg(clap::Arg::with_name("datadir")
            .help("Data directory (default is ~/.config/vocajeux/data/")
            .short("d")
            .long("dir")
            .takes_value(true)
            .default_value(defaultdatadir.to_str().unwrap())
        )
        .arg(clap::Arg::with_name("sessiondir")
            .help("Session directory (default is ~/.config/vocajeux/scores/")
            .short("S")
            .long("sessiondir")
            .takes_value(true)
            .default_value(defaultsessiondir.to_str().unwrap())
        )
        .arg(clap::Arg::with_name("eval")
            .help("Evaluate a statement, multiple statement can be separated by a semicolon")
            .short("e")
            .long("eval")
            .takes_value(true))
        .get_matches();

    let sessiondir: &str = argmatches.value_of("sessiondir").unwrap();
    let datadir: &str = argmatches.value_of("datadir").unwrap();
    if !PathBuf::from(sessiondir).exists() {
        fs::create_dir_all(&sessiondir).expect("Unable to create session directory");
    }
    if !PathBuf::from(datadir).exists() {
        fs::create_dir_all(&datadir).expect("Unable to create data directory");
    }

    let mut opt_session: Option<VocaSession> = None;
    if let Some(eval) = argmatches.value_of("eval") {
        let script: Vec<String> = eval.split(";").map(|s| s.trim().to_owned() ).collect();
        for statement in script {
            opt_session = if let Some(mut session) = opt_session {
                if !session.handle_response(statement.clone(), datadir, sessiondir) {
                    handle_response(statement, Some(session), datadir, sessiondir )
                } else {
                    Some(session)
                }
            } else {
                handle_response(statement, opt_session, datadir, sessiondir )
            }

        }
    }

    loop {
        opt_session = if let Some(mut session) = opt_session {
            if let Some(response) = session.prompt() {
                if !session.handle_response(response.clone(), datadir, sessiondir) {
                    handle_response(response, Some(session), datadir, sessiondir )
                } else {
                    Some(session)
                }
            } else {
                Some(session)
            }
        } else {
            if let Some(response) = getinputline() {
                handle_response(response, opt_session, datadir, sessiondir )
            } else {
                opt_session
            }
        }
    }
}
