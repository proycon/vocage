extern crate rand;
extern crate chrono;
extern crate ansi_term;
//extern crate clap;

use std::fs::File;
use std::io::{Write,Read,BufReader,BufRead,Error,ErrorKind};
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt;
use rand::prelude::{thread_rng,Rng};
use chrono::NaiveDateTime;
use ansi_term::Colour;
//use clap::Arg;

pub struct VocaSession {
    columns: Vec<String>,
    decks: Vec<String>,
    ///interval in minutes
    intervals: Vec<u32>,
    returntofirst: bool,
    filename: String,
    ///Configuration of columns to show for each side of the card
    showcolumns: Vec<Vec<u8>>,
    ///list delimiter
    listdelimiter: Option<String>,
    header: bool,
}

pub struct VocaData {
    session: VocaSession,
    cards: Vec<VocaCard>,
}

pub struct VocaCard {
    fields: Vec<String>,
    due: Option<NaiveDateTime>,
    deck: u8
}

#[derive(Debug,Copy,Clone)]
pub enum PrintFormat {
    Plain,
    AnsiColour
}

impl VocaData {
    pub fn from_file(filename: &str) -> Result<Self,std::io::Error> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut cards: Vec<VocaCard> = Vec::new();
        let mut columns: Vec<String> = Vec::new();
        let mut decks: Vec<String> = Vec::new();
        let mut intervals: Vec<u32> = Vec::new();
        let mut showcolumns: Vec<Vec<u8>> = Vec::new();
        let mut listdelimiter: Option<String> = None;
        let mut header: bool = false;
        let mut columncount: u8 = 0;
        let mut returntofirst = false;
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if line.starts_with('#') {
                //metadata or comment
                if line.starts_with("#decks:") {
                    decks = line[7..].trim().split(",").map(|s| s.trim().to_owned()).collect();
                } else if line.starts_with("#intervals:") {
                    intervals = line[11..].trim().split(",").map(|s| s.parse::<u32>().expect("parsing interval")).collect();
                } else if line.starts_with("#columns:") {
                    columns = line[9..].trim().split(",").map(|s| s.trim().to_owned()).collect();
                } else if line.starts_with("#showcolumns:") {
                    //may be specified multiple times for multiple 'sides' of the card
                    showcolumns.push(line[13..].trim().split(",").map( |s|
                        columns.iter().enumerate() .find(|&r| r.1 == s.trim() )
                        .expect(format!("showcolumns references a non-existing column: {}",s).as_str())
                        .0
                    ).map(|n| n as u8 ).collect());
                } else if line.starts_with("#listdelimiter:") {
                    listdelimiter = Some(line[15..].trim().to_owned());
                } else if line == "#returntofirst" {
                    returntofirst = true;
                }
            } else if !line.is_empty() {
                let card = VocaCard::parse_line(&line)?;
                if i == 0 && !line.contains("deck#") && !line.contains("due@") && line == line.to_uppercase() {
                    columns = card.fields;
                    header = true
                } else {
                    let length = card.fields.len() as u8;
                    if length > columncount {
                       columncount = length;
                    }
                    cards.push(card);
                }
            }
        }
        if showcolumns.is_empty() {
            //default configuration: two sides
            showcolumns.push(vec!(0)); //first column on front side
            showcolumns.push((1..columncount).collect()); //other columns on back side
        }
        Ok(VocaData {
            cards: cards,
            session: VocaSession {
                columns: columns,
                decks: decks,
                intervals: intervals,
                returntofirst: returntofirst,
                filename: filename.to_owned(),
                showcolumns: showcolumns,
                listdelimiter: listdelimiter,
                header: header
            }
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


    pub fn write(&self) -> Result<(),std::io::Error> {
        let mut file = std::fs::File::create(self.session.filename.as_str())?;
        //contents
        if self.session.header {
            file.write(self.session.columns.join("\t").as_bytes() )?;
            file.write(b"\n")?;
        }
        for card in self.cards.iter() {
            file.write(card.write_to_string().as_bytes())?;
            file.write(b"\n")?;
        }
        //metadata last
        file.write(b"#METADATA:\n")?; //end of metadata
        if !self.session.decks.is_empty() {
            file.write(b"#decks: ")?;
            file.write(self.session.decks.join(",").as_bytes() )?;
            file.write(b"\n")?;
        }
        if !self.session.intervals.is_empty() {
            file.write(b"#intervals: ")?;
            file.write(self.session.intervals.iter().map(|s| format!("{}",s)).collect::<Vec<String>>().join(",").as_bytes() )?;
            file.write(b"\n")?;
        }
        if let Some(listdelimiter) = &self.session.listdelimiter {
            file.write(b"#listdelimiter: ")?;
            file.write(listdelimiter.as_bytes())?;
            file.write(b"\n")?;
        }
        if self.session.returntofirst {
            file.write(b"#returntofirst\n")?;
        }
        if !self.session.columns.is_empty() {
            if !self.session.header {
                file.write(b"#columns: ")?;
                file.write(self.session.columns.join(",").as_bytes() )?;
                file.write(b"\n")?;
            }
            for showcolumns in self.session.showcolumns.iter() {
                file.write(b"#showcolumns: ")?;
                file.write(showcolumns.iter().map(|n| format!("{}", self.session.columns[*n as usize]).to_string()).collect::<Vec<String>>().join(",").as_bytes() )?;
                file.write(b"\n")?;
            }
        }
        Ok(())
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

    pub fn write_to_string(&self) -> String {
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

    pub fn move_to_deck(&mut self, deck: u8, session: &VocaSession)  {
        if let Some(interval) = session.intervals.get(deck as usize) {
            self.due = Some(NaiveDateTime::from_timestamp(
                SystemTime::now().duration_since(UNIX_EPOCH).expect("Unable to get time").as_secs() as i64 + (interval * 60) as i64, 0
            ));
        }
        self.deck = deck;
    }

    pub fn correct(&mut self, session: &VocaSession) {
        if ((self.deck+1) as usize) < session.decks.len() {
            self.move_to_deck(self.deck+1, session);
        } else {
            self.move_to_deck(self.deck, session);
        }
    }

    pub fn incorrect(&mut self, session: &VocaSession) {
        if self.deck > 0 && !session.returntofirst {
            self.move_to_deck(self.deck-1, session);
        } else {
            self.move_to_deck(0, session);
        }
    }

    pub fn print_to_string(&self, side: u8, session: &VocaSession, format: PrintFormat, wraplist: bool) -> Result<String, std::fmt::Error> {
        if let Some(showcolumns) = session.showcolumns.get(side as usize) {
            let mut output: String = String::new();
            for showcolumn in showcolumns.iter() {
                let field = self.print_field_to_string(*showcolumn, session, format, wraplist)?;
                output += field.as_str();
                output += "\n";
            }
            Ok(output)
        } else {
            Err(fmt::Error)
        }
    }

    pub fn print_field_to_string(&self, index: u8, session: &VocaSession, format: PrintFormat, wraplist: bool) -> Result<String, std::fmt::Error> {
        if let Some(field) = self.fields.get(index as usize) {
            let output = if let Some(listdelimiter) = &session.listdelimiter {
                if wraplist {
                    field.replace(listdelimiter.as_str(), "\n")
                } else {
                    field.clone() //maybe TODO: use Cow instead of cloning for better performance
                }
            } else {
                field.clone()
            };
            match format {
                PrintFormat::Plain => Ok(output),
                PrintFormat::AnsiColour => {
                    match index {
                        0 => Ok(Colour::Green.paint(output).to_string()),
                        1 => Ok(Colour::Cyan.paint(output).to_string()),
                        2 => Ok(Colour::Yellow.paint(output).to_string()),
                        3 => Ok(Colour::Purple.paint(output).to_string()),
                        4 => Ok(Colour::Blue.paint(output).to_string()),
                        _ => Ok(output),
                    }
                }
            }
        } else {
            Ok(String::new()) //empty string
        }
    }

}

pub fn getinputline() -> Option<String> {
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
