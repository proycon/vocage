extern crate rand;
extern crate chrono;

use std::fs::File;
use std::io::{Write,Read,BufReader,BufRead,Error,ErrorKind};
use std::time::{SystemTime, UNIX_EPOCH};
use rand::prelude::{thread_rng,Rng};
use chrono::NaiveDateTime;


pub struct VocaData {
    header: Vec<String>,
    cards: Vec<VocaCard>,
    decks: Vec<String>,
    //interval in minutes
    intervals: Vec<u32>,
}

pub struct VocaCard {
    fields: Vec<String>,
    due: Option<NaiveDateTime>,
    deck: u8
}

impl VocaData {
    pub fn from_file(filename: &str) -> Result<Self,std::io::Error> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut cards: Vec<VocaCard> = Vec::new();
        let mut header: Vec<String> = Vec::new();
        let mut decks: Vec<String> = Vec::new();
        let mut intervals: Vec<u32> = Vec::new();
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if i == 0 && !line.is_empty() && !line.contains("deck#") && !line.contains("due@") {
                //header
                header = VocaCard::parse_line(&line)?.fields;
            }
            if line.starts_with('#') {
                //metadata or comment
                if line.starts_with("#decks:") {
                    decks = line[7..].trim().split(",").map(|s| s.to_owned()).collect();
                } else if line.starts_with("#intervals:") {
                    intervals = line[11..].trim().split(",").map(|s| s.parse::<u32>().expect("parsing interval")).collect();
                }
            } else if !line.is_empty() {
                cards.push(VocaCard::parse_line(&line)?);
            }
        }
        Ok(VocaData {
            header: header,
            cards: cards,
            decks: decks,
            intervals: intervals,
        })
    }

    pub fn random_index(&self, rng: &mut impl Rng, deck: Option<u8>, due_only: bool) -> Option<usize> {
        let mut indices: Vec<usize> = Vec::new();

        let now: NaiveDateTime = NaiveDateTime::from_timestamp(
            SystemTime::now().duration_since(UNIX_EPOCH).expect("Unable to get time").as_secs() as i64, 0
        );

        for (i, card) in self.cards.iter().enumerate() {
            if deck.is_none() || card.deck == deck.unwrap() {
                if !due_only || (due_only && (card.due.is_none() || card.due.unwrap() < now)) {
                    indices.push(i);
                }
            }
        }

        if !indices.is_empty() {
            return Some(rng.gen_range(0, indices.len()));
        }
        None
    }

    pub fn pick_card<'a>(&'a self, rng: &mut impl Rng, deck: Option<u8>, due_only: bool) -> Option<&'a VocaCard> {
        if let Some(choice) = self.random_index(rng, deck, due_only) {
            return Some(&self.cards[choice]);
        }
        None
    }

    pub fn pick_card_mut<'a>(&'a mut self, rng: &mut impl Rng, deck: Option<u8>, due_only: bool) -> Option<&'a mut VocaCard> {
        if let Some(choice) = self.random_index(rng, deck, due_only) {
            return Some(&mut self.cards[choice]);
        }
        None
    }
}


impl VocaCard {
    pub fn parse_line(line: &str) -> Result<VocaCard, std::io::Error> {
        let mut prevc: char = 0 as char;
        let mut begin = 0;
        let mut fields: Vec<String> =  Vec::new();
        let mut deck: u8 = 0;
        let mut due: Option<NaiveDateTime> = None;
        let length = line.chars().count();
        for (i, c) in line.chars().enumerate() {
            if (i == length -1) || ((prevc == ' ' || prevc == '\t') && (c == ' ' || c == '\t'))  {
                //handle previous column
                let value = &line[begin..if i == length - 1 {
                    length
                } else {
                    i
                }];
                if value.starts_with("deck#") {
                    if let Ok(num) = &value[5..].parse() {
                        deck = *num;
                    }
                } else if value.starts_with("due@") {
                    due = match NaiveDateTime::parse_from_str(&value[4..], "%Y-%m-%d %H:%M:%S") {
                        Ok(dt) => Some(dt),
                        Err(e) => {
                            return Err(std::io::Error::new(ErrorKind::InvalidData, format!("Unable to parse due date: {}",e)));
                        }
                    };
                } else {
                    if value == "-" { //empty field placeholder
                        fields.push(String::new());
                    } else {
                        fields.push(value.to_owned());
                    }
                }
                begin = i
            }
            prevc = c;
        }
        Ok( VocaCard {
            fields: fields,
            due: due,
            deck: deck
        })
    }

    pub fn output(&self) -> String {
        let mut result: String = String::new();
        for (i, field) in self.fields.iter().enumerate() {
            if i > 0 {
                result += "\t";
            }
            if field.is_empty() {
                result += "-";
            }
        }
        if self.deck > 0 {
            result = format!("{}\tdeck#{}",result, self.deck);
        }
        if let Some(due) = self.due {
            result = format!("{}\tdue@{}",result, due.format("%Y-%m-%d %H:%M:%S").to_string().as_str() );
        }
        result
    }

    pub fn move_to_deck(&mut self, deck: u8, intervals: &Vec<u32>)  {
    }
}
