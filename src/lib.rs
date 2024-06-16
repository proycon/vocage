extern crate ansi_term;
extern crate chrono;
extern crate clap;
extern crate rand;

use ansi_term::Colour;
use chrono::NaiveDateTime;
use clap::{App, Arg};
use rand::prelude::Rng;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader, Error, ErrorKind, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct VocaSession {
    pub columns: Vec<String>,
    pub decks: Vec<String>,
    ///interval in minutes
    pub intervals: Vec<u32>,
    pub returntofirst: bool,
    filename: Option<String>,
    ///Configuration of columns to show for each side of the card
    pub showcolumns: Vec<Vec<u8>>,
    ///list delimiter
    pub listdelimiter: Option<String>,
    header: bool,
}

pub struct VocaData {
    pub session: VocaSession,
    pub cards: Vec<VocaCard>,
    pub comments: Vec<(usize, String)>,
}

pub struct VocaCard {
    pub fields: Vec<String>,
    pub due: Option<NaiveDateTime>,
    pub deck: u8,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PrintFormat {
    Plain,
    AnsiColour,
}

impl VocaSession {
    pub fn common_arguments<'a, 'b>() -> Vec<clap::Arg<'a, 'b>> {
        let mut args: Vec<Arg> = Vec::new();
        args.push( Arg::with_name("showcolumns")
            .long("showcolumns")
            .short("-C")
            .help("Specify what columns to show on the card, comma separated list of column names, specify this multiple times to define multiple 'sides' of the card. The columns themselves are defined using --columns")
            .multiple(true)
            .takes_value(true)
        );
        args.push(
            Arg::with_name("decks")
                .long("decks")
                .short("-d")
                .help("Comma seperated list of deck names")
                .takes_value(true),
        );
        args.push( Arg::with_name("intervals")
            .long("intervals")
            .short("-i")
            .help("Comma seperated list of intervals for each respective deck (in minutes). Must contain as many items as --decks")
            .takes_value(true)
        );
        args.push(
            Arg::with_name("columns")
                .long("columns")
                .short("-c")
                .help("Comma separated list of column names.")
                .takes_value(true),
        );
        args.push( Arg::with_name("listdelimiter")
            .long("listdelimiter")
            .short("-l")
            .help("List delimiter to separate multiple alternatives within a field (recommended: | )")
            .takes_value(true)
        );
        args.push( Arg::with_name("returntofirst")
            .long("returntofirst")
            .short("-1")
            .help("When a card is demoted (e.g. answered incorrectly), demote it to the very first deck rather than the previous deck")
        );
        args
    }

    pub fn set_common_arguments<'a>(&mut self, args: &clap::ArgMatches<'a>) -> Result<(), Error> {
        if let Some(decks) = args.value_of("decks") {
            self.decks = decks
                .trim()
                .split(",")
                .map(|s| s.trim().to_owned())
                .collect();
        }
        if let Some(intervals) = args.value_of("intervals") {
            self.intervals = intervals
                .trim()
                .split(",")
                .map(|s| s.parse::<u32>().expect("parsing interval"))
                .collect();
        }
        if let Some(columns) = args.value_of("columns") {
            self.columns = columns
                .trim()
                .split(",")
                .map(|s| s.trim().to_owned())
                .collect();
        }
        if let Some(listdelimiter) = args.value_of("listdelimiter") {
            self.listdelimiter = Some(listdelimiter.to_string());
        }
        if let Some(showcolumns) = args.values_of("showcolumns") {
            self.showcolumns.clear();
            for showcolumns in showcolumns {
                self.showcolumns.push(
                    showcolumns
                        .trim()
                        .split(",")
                        .map(|s| {
                            self.columns
                                .iter()
                                .enumerate()
                                .find(|&r| r.1 == s.trim())
                                .expect(
                                    format!(
                                        "ERROR: showcolumns references a non-existing column: {}",
                                        s
                                    )
                                    .as_str(),
                                )
                                .0
                        })
                        .map(|n| n as u8)
                        .collect(),
                );
            }
        }
        if args.is_present("returntofirst") {
            self.returntofirst = true;
        }

        //sanity checks and defaults
        if self.decks.len() > 0 && self.intervals.is_empty() {
        } else if self.decks.len() != self.intervals.len() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "ERROR: intervals and decks have different length",
            ));
        }

        if self.showcolumns.is_empty() {
            //default configuration: two sides
            self.showcolumns.push(vec![0]); //first column on front side
            self.showcolumns
                .push((1..self.columns.len()).map(|n| n as u8).collect()); //other columns on back side
        }
        Ok(())
    }

    pub fn from_arguments(args: Vec<&str>) -> Result<Self, Error> {
        let mut vocasession = Self::default();
        let mut args = args.clone();
        args.insert(0, "metadata");
        let args = App::new("metadata")
            .args(&Self::common_arguments())
            .get_matches_from(args);
        vocasession.set_common_arguments(&args)?;
        Ok(vocasession)
    }

    pub fn get_deck_by_name(&self, name: &str) -> Option<u8> {
        for (i, n) in self.decks.iter().enumerate() {
            if n == name {
                return Some(i as u8);
            }
        }
        None
    }
}

impl Default for VocaSession {
    fn default() -> Self {
        VocaSession {
            columns: Vec::new(),
            decks: Vec::new(),
            intervals: Vec::new(),
            returntofirst: false,
            filename: None,
            showcolumns: Vec::new(),
            listdelimiter: None,
            header: false,
        }
    }
}

impl VocaData {
    pub fn from_file(filename: &str, reset: bool) -> Result<Self, std::io::Error> {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);
        let mut cards: Vec<VocaCard> = Vec::new();
        let mut comments: Vec<(usize, String)> = Vec::new();
        let mut header: bool = false;
        let mut columncount: u8 = 0;
        let mut metadata_args: Vec<String> = vec![];
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            if line.starts_with('#') {
                //metadata or comment
                if line.starts_with("#--") {
                    //metadata
                    if let Some((index, _)) = line.chars().enumerate().find(|&r| r.1 == ' ') {
                        metadata_args.push(format!("--{}", &line[3..index]).to_owned());
                        metadata_args.push(line[index + 1..].to_owned());
                    } else {
                        metadata_args.push(format!("--{}", &line[3..]).to_owned());
                    }
                } else {
                    comments.push((cards.len(), line)); //we store the index so we can later serialise it in proper order again
                }
            } else if !line.is_empty() {
                let card = VocaCard::parse_line(&line, reset, i + 1)?;
                if i == 0
                    && !line.contains("deck#")
                    && !line.contains("due@")
                    && line == line.to_uppercase()
                {
                    metadata_args.push("--columns".to_owned());
                    metadata_args.push(card.fields.join(","));
                    header = true
                } else {
                    let length = card.fields.len() as u8;
                    if length > columncount {
                        columncount = length;
                    }
                    cards.push(card);
                }
            } else {
                //empty lines are considered comments for our purposes, we retain them in the output
                comments.push((cards.len(), line));
            }
        }
        if !metadata_args.contains(&"--columns".to_owned()) {
            //no column/header information provided, infer
            metadata_args.push("--columns".to_owned());
            metadata_args.push(
                (1..=columncount)
                    .map(|n| format!("column#{}", n).to_owned())
                    .collect::<Vec<String>>()
                    .join(","),
            );
        }
        let mut session =
            VocaSession::from_arguments(metadata_args.iter().map(|s| s.as_str()).collect())?;
        session.header = header;
        session.filename = Some(filename.to_owned());

        Ok(VocaData {
            cards: cards,
            session: session,
            comments: comments,
        })
    }

    pub fn random_index(
        &self,
        rng: &mut impl Rng,
        decks: Option<&Vec<u8>>,
        due_only: bool,
        seen_only: bool,
    ) -> Option<(usize, usize)> {
        let mut indices: Vec<usize> = Vec::new();

        let now: NaiveDateTime = NaiveDateTime::from_timestamp(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Unable to get time")
                .as_secs() as i64,
            0,
        );

        for (i, card) in self.cards.iter().enumerate() {
            if card.is_presentable(Some(&now), decks, due_only, seen_only) {
                indices.push(i);
            }
        }

        if !indices.is_empty() {
            return Some((indices[rng.gen_range(0, indices.len())], indices.len()));
        }
        None
    }

    pub fn next_index(
        &self,
        index: usize,
        decks: Option<&Vec<u8>>,
        due_only: bool,
        seen_only: bool,
        inclusive: bool,
    ) -> Option<(usize, usize)> {
        let now: NaiveDateTime = NaiveDateTime::from_timestamp(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Unable to get time")
                .as_secs() as i64,
            0,
        );

        let mut next: Option<usize> = None;
        let mut count: usize = 0;
        for (i, card) in self.cards.iter().enumerate() {
            if (!inclusive && i > index) || (inclusive && i >= index) {
                if card.is_presentable(Some(&now), decks, due_only, seen_only) {
                    if next.is_none() {
                        next = Some(i);
                    } else {
                        count += 1;
                    }
                }
            }
        }
        next.map(|i| (i, count))
    }

    pub fn pick_card<'a>(
        &'a self,
        rng: &mut impl Rng,
        decks: Option<&Vec<u8>>,
        due_only: bool,
        seen_only: bool,
    ) -> Option<&'a VocaCard> {
        if let Some((choice, _)) = self.random_index(rng, decks, due_only, seen_only) {
            return Some(&self.cards[choice]);
        }
        None
    }

    pub fn pick_card_mut<'a>(
        &'a mut self,
        rng: &mut impl Rng,
        decks: Option<&Vec<u8>>,
        due_only: bool,
        seen_only: bool,
    ) -> Option<&'a mut VocaCard> {
        if let Some((choice, _)) = self.random_index(rng, decks, due_only, seen_only) {
            return Some(&mut self.cards[choice]);
        }
        None
    }

    pub fn pick_next_card<'a>(
        &'a self,
        index: usize,
        decks: Option<&Vec<u8>>,
        due_only: bool,
        seen_only: bool,
        inclusive: bool,
    ) -> Option<&'a VocaCard> {
        if let Some((choice, _)) = self.next_index(index, decks, due_only, seen_only, inclusive) {
            return Some(&self.cards[choice]);
        }
        None
    }

    pub fn pick_next_card_mut<'a>(
        &'a mut self,
        index: usize,
        decks: Option<&Vec<u8>>,
        due_only: bool,
        seen_only: bool,
        inclusive: bool,
    ) -> Option<&'a mut VocaCard> {
        if let Some((choice, _)) = self.next_index(index, decks, due_only, seen_only, inclusive) {
            return Some(&mut self.cards[choice]);
        }
        None
    }

    pub fn write(&self, reset: bool) -> Result<(), std::io::Error> {
        if self.session.filename.is_none() {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "No filename configured",
            ));
        }
        let mut file = std::fs::File::create(self.session.filename.as_ref().unwrap().as_str())?;
        //contents
        if self.session.header {
            file.write(self.session.columns.join("\t").as_bytes())?;
            file.write(b"\n")?;
        }
        let mut nextcommentindex = if !self.comments.is_empty() {
            //initialise
            Some(self.comments[0].0)
        } else {
            None
        };
        for (i, card) in self.cards.iter().enumerate() {
            //make sure to process very first comments
            if i == 0 && nextcommentindex.is_some() && nextcommentindex.unwrap() == 0 {
                for (commentindex, comment) in self.comments.iter() {
                    if *commentindex == 0 {
                        file.write(comment.as_bytes())?;
                        file.write(b"\n")?;
                        nextcommentindex = None; //reset
                    } else if *commentindex > 0 {
                        nextcommentindex = Some(*commentindex); //set for next
                        break;
                    }
                }
            }
            file.write(
                card.write_to_string(self.session.columns.len(), reset)
                    .as_bytes(),
            )?;
            file.write(b"\n")?;
            //process remaining comments
            if nextcommentindex.is_some() && i + 1 == nextcommentindex.unwrap() {
                for (commentindex, comment) in self.comments.iter() {
                    if *commentindex == i + 1 {
                        file.write(comment.as_bytes())?;
                        file.write(b"\n")?;
                        nextcommentindex = None; //reset
                    } else if *commentindex > i + 1 {
                        nextcommentindex = Some(*commentindex); //set for next
                        break;
                    }
                }
            }
        }
        //metadata last
        if !self.session.decks.is_empty() {
            file.write(b"#--decks ")?;
            file.write(self.session.decks.join(",").as_bytes())?;
            file.write(b"\n")?;
        }
        if !self.session.intervals.is_empty() {
            file.write(b"#--intervals ")?;
            file.write(
                self.session
                    .intervals
                    .iter()
                    .map(|s| format!("{}", s))
                    .collect::<Vec<String>>()
                    .join(",")
                    .as_bytes(),
            )?;
            file.write(b"\n")?;
        }
        if let Some(listdelimiter) = &self.session.listdelimiter {
            file.write(b"#--listdelimiter ")?;
            file.write(listdelimiter.as_bytes())?;
            file.write(b"\n")?;
        }
        if self.session.returntofirst {
            file.write(b"#--returntofirst\n")?;
        }
        if !self.session.columns.is_empty() {
            if !self.session.header {
                file.write(b"#--columns ")?;
                file.write(self.session.columns.join(",").as_bytes())?;
                file.write(b"\n")?;
            }
            for showcolumns in self.session.showcolumns.iter() {
                file.write(b"#--showcolumns ")?;
                file.write(
                    showcolumns
                        .iter()
                        .map(|n| format!("{}", self.session.columns[*n as usize]).to_string())
                        .collect::<Vec<String>>()
                        .join(",")
                        .as_bytes(),
                )?;
                file.write(b"\n")?;
            }
        }
        Ok(())
    }
}

impl VocaCard {
    pub fn parse_line(line: &str, reset: bool, linenr: usize) -> Result<VocaCard, std::io::Error> {
        let mut begin = 0;
        let mut fields: Vec<String> = Vec::new();
        let mut deck: u8 = 0;
        let mut due: Option<NaiveDateTime> = None;
        let length = line.chars().count();
        for (j, (i, c)) in line.char_indices().enumerate() {
            if (j == length - 1) || (c == '\t') {
                //handle previous column
                let value = &line[begin..if j == length - 1 { line.len() } else { i }];
                if value.starts_with("deck#") {
                    if !reset {
                        if let Ok(num) = &value[5..].parse::<u8>() {
                            deck = *num - 1;
                        }
                    }
                } else if value.starts_with("due@") {
                    if !reset {
                        due = match NaiveDateTime::parse_from_str(&value[4..], "%Y-%m-%d %H:%M:%S")
                        {
                            Ok(dt) => Some(dt),
                            Err(e) => {
                                return Err(std::io::Error::new(
                                    ErrorKind::InvalidData,
                                    format!("Unable to parse due date on line {}: {}", linenr, e),
                                ));
                            }
                        };
                    }
                } else {
                    if value.is_empty() || value == "-" {
                        //empty field placeholder
                        fields.push(String::new());
                    } else {
                        fields.push(value.trim().to_owned());
                    }
                }
                begin = i + 1
            }
        }
        Ok(VocaCard {
            fields: fields,
            due: due,
            deck: deck,
        })
    }

    pub fn write_to_string(&self, columncount: usize, reset: bool) -> String {
        let mut result: String = String::new();
        for (i, field) in self.fields.iter().enumerate() {
            if field.is_empty() && i >= columncount {
                //empty placeholder fields for deck and due
                break;
            }
            if i > 0 {
                result += "\t";
            }
            if field.is_empty() || field == "-" {
                result += "";
            } else {
                result += field;
            }
        }
        for _ in self.fields.len()..columncount {
            result += "\t";
        }
        if !reset {
            if self.deck > 0 {
                result = format!("{}\tdeck#{}", result, self.deck + 1);
            } else {
                result += "\t";
            }
            if let Some(due) = self.due {
                result = format!(
                    "{}\tdue@{}",
                    result,
                    due.format("%Y-%m-%d %H:%M:%S").to_string().as_str()
                );
            } else {
                result += "\t";
            }
        }
        result
    }

    pub fn move_to_deck(&mut self, deck: u8, session: &VocaSession) -> bool {
        if deck >= session.decks.len() as u8 {
            return false;
        }
        if let Some(interval) = session.intervals.get(deck as usize) {
            self.due = Some(NaiveDateTime::from_timestamp(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Unable to get time")
                    .as_secs() as i64
                    + (interval * 60) as i64,
                0,
            ));
        }
        self.deck = deck;
        true
    }

    pub fn promote(&mut self, session: &VocaSession) -> bool {
        if ((self.deck + 1) as usize) < session.decks.len() {
            self.move_to_deck(self.deck + 1, session);
            true
        } else {
            self.move_to_deck(self.deck, session);
            false
        }
    }

    pub fn demote(&mut self, session: &VocaSession) -> bool {
        if self.deck > 0 && !session.returntofirst {
            self.move_to_deck(self.deck - 1, session);
            true
        } else {
            self.move_to_deck(0, session);
            false
        }
    }

    pub fn print(
        &self,
        side: u8,
        session: &VocaSession,
        format: PrintFormat,
        wraplist: bool,
    ) -> Result<(), std::fmt::Error> {
        let output = self.fields_to_str(side, session, wraplist)?;
        for (index, output) in output {
            match format {
                PrintFormat::Plain => println!("{}", output),
                PrintFormat::AnsiColour => match index {
                    0 => println!("{}", Colour::Green.paint(output).to_string()),
                    1 => println!("{}", Colour::Cyan.paint(output).to_string()),
                    2 => println!("{}", Colour::Yellow.paint(output).to_string()),
                    3 => println!("{}", Colour::Purple.paint(output).to_string()),
                    4 => println!("{}", Colour::Blue.paint(output).to_string()),
                    _ => println!("{}", output),
                },
            }
        }
        Ok(())
    }

    pub fn fields_to_str(
        &self,
        side: u8,
        session: &VocaSession,
        wraplist: bool,
    ) -> Result<Vec<(u8, &str)>, std::fmt::Error> {
        if let Some(showcolumns) = session.showcolumns.get(side as usize) {
            let mut output: Vec<(u8, &str)> = Vec::new();
            for showcolumn in showcolumns.iter() {
                let lines = self.field_to_str(*showcolumn, session, wraplist)?;
                for line in lines {
                    output.push((*showcolumn, line));
                }
            }
            Ok(output)
        } else {
            Err(fmt::Error)
        }
    }

    pub fn field_to_str(
        &self,
        index: u8,
        session: &VocaSession,
        wraplist: bool,
    ) -> Result<Vec<&str>, std::fmt::Error> {
        if let Some(field) = self.fields.get(index as usize) {
            let output: Vec<&str> = if let Some(listdelimiter) = &session.listdelimiter {
                if wraplist {
                    field.split(listdelimiter.as_str()).collect()
                } else {
                    vec![field.as_str()]
                }
            } else {
                vec![field.as_str()]
            };
            Ok(output)
        } else {
            Ok(Vec::new()) //empty string
        }
    }

    pub fn is_presentable(
        &self,
        now: Option<&NaiveDateTime>,
        decks: Option<&Vec<u8>>,
        due_only: bool,
        seen_only: bool,
    ) -> bool {
        let now: NaiveDateTime = match now {
            Some(dt) => *dt,
            None => NaiveDateTime::from_timestamp(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("Unable to get time")
                    .as_secs() as i64,
                0,
            ),
        };
        if decks.is_none() || decks.unwrap().contains(&self.deck) {
            if self.due.is_none() && seen_only {
                return false;
            }
            if !due_only || (due_only && (self.due.is_none() || self.due.unwrap() < now)) {
                return true;
            }
        }
        false
    }
}

pub fn load_files(files: Vec<&str>, force: bool, reset: bool) -> Vec<VocaData> {
    let mut datasets: Vec<VocaData> = Vec::new();

    for filename in files.iter() {
        if !PathBuf::from(filename).exists() {
            eprintln!("ERROR: Specified input file not does exist: {}", filename);
            std::process::exit(1);
        } else {
            match VocaData::from_file(filename, reset) {
                Ok(mut data) => {
                    if !datasets.is_empty() {
                        if data.session.columns != datasets[0].session.columns {
                            eprintln!("ERROR: columns of {} differ from those in the first loaded file, unable to load together.", filename);
                            std::process::exit(1);
                        }
                        if data.session.decks != datasets[0].session.decks {
                            if force || data.session.decks.is_empty() {
                                data.session.decks = datasets[0].session.decks.clone();
                                data.session.intervals = datasets[0].session.intervals.clone();
                            } else {
                                eprintln!("ERROR: decks of {} differ from those in the first loaded file, refusing to load together (use --force to force it)", filename);
                                std::process::exit(1);
                            }
                        }
                    }
                    datasets.push(data);
                }
                Err(err) => {
                    eprintln!("ERROR loading {}: {}", filename, err);
                    std::process::exit(1);
                }
            }
        }
    }

    datasets
}
