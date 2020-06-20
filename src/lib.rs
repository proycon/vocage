extern crate rand;
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;
#[macro_use]
extern crate serde_derive;
extern crate regex;
extern crate md5;
extern crate dirs;
extern crate csv;
#[macro_use]
extern crate simple_error;

use std::fs;
use std::error::Error;
use std::fmt;
use std::io;
use std::iter::Iterator;
use std::collections::{HashMap,HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use md5::{compute,Digest};
use std::path::{Path,PathBuf};
use std::iter::FromIterator;
use rand::seq::SliceRandom;
use rand::{thread_rng,Rng};
use self::simple_error::SimpleError;


/// Vocabulary Item data structure
#[derive(Serialize, Deserialize)]
pub struct VocaCard {
    #[serde(skip)]
    pub id: String,
    #[serde(default)] //deserialise missing fields to default empty values
    pub words: Vec<String>,
    #[serde(default)]
    pub transcriptions: Vec<String>,
    #[serde(default)]
    pub translations: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub comments: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>
}

/// Vocabulary List data structure
#[derive(Serialize, Deserialize)]
pub struct VocaSet {
    #[serde(skip)]
    pub filename: String,
    pub cards: Vec<VocaCard>
}

#[derive(Serialize, Deserialize)]
pub struct VocaSession {
    ///Filename of the session
    pub filename: String,
    ///Filename of the vocabulary set
    pub set_filename: String,
    ///Deck names
    pub deck_names: Vec<String>,
    ///mapping of deck index to vocacard IDs
    pub decks: Vec<Vec<String>>,
    ///Number of times answered correctly (i.e. moved to the next deck)
    pub correct: HashMap<String,u32>,
    ///Number of times answered incorrectly (i.e. moved to the previous deck()
    pub incorrect: HashMap<String,u32>,
    ///Last presentation by random pick method
    pub lastvisit: HashMap<String,u64>,
    pub mode: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub correct_option: usize,
    #[serde(default)]
    pub deck_index: Option<usize>, //the selected deck
    #[serde(default)]
    pub card_index: Option<usize>, //the selected card
    #[serde(default)]
    pub settings: HashSet<String>,
    #[serde(default)]
    pub settings_int: HashMap<String, usize>,
    #[serde(default)]
    pub settings_str: HashMap<String, String>,
    #[serde(skip)]
    pub set: Option<VocaSet>,
}

#[derive(Serialize, Deserialize,Debug,Clone,Copy)]
pub enum FieldType {
    Word,
    Example,
    Phon, //transcription
    Translation,
    Comment,
    Tag
}

#[derive(Serialize, Deserialize,Debug,Clone)]
pub enum Filter {
    None,
    Any(Vec<Filter>),
    All(Vec<Filter>),
    Not(Box<Filter>),
    Equals(FieldType,String),
}

impl Filter {
    fn matches(&self, card: &VocaCard) -> bool {
        match self {
            Filter::None => true,
            Filter::Any(v) => {
                v.iter().any(|f| f.matches(card))
            },
            Filter::All(v) => {
                v.iter().all(|f| f.matches(card))
            },
            Filter::Not(f) => {
                !f.matches(card)
            },
            Filter::Equals(field,value) => {
                match field {
                    FieldType::Word => {
                        card.words.contains(&value)
                    },
                    FieldType::Phon => {
                        card.transcriptions.contains(&value)
                    },
                    FieldType::Example => {
                        card.examples.contains(&value)
                    },
                    FieldType::Comment => {
                        card.comments.contains(&value)
                    },
                    FieldType::Translation => {
                        card.translations.contains(&value)
                    },
                    FieldType::Tag => {
                        card.tags.contains(&value)
                    }
                }
            },
        }
    }

    fn parse(query: &str) -> Result<Filter,SimpleError> {
        if query.is_empty() {
            Ok(Filter::None)
        } else {
            if query.starts_with("any(") && query.ends_with(")") {
                let subqueries: Vec<&str> = query[5..query.len()-1].split(",").collect();
                let mut filters: Vec<Filter> = Vec::new();
                for subquery in subqueries.iter() {
                    match Filter::parse(subquery)  {
                        Ok(filter) => filters.push(filter),
                        Err(err) => return Err(err),
                    }
                }
                Ok(Filter::Any(filters))
            } else if query.starts_with("all(") && query.ends_with(")") {
                let subqueries: Vec<&str> = query[5..query.len()-1].split(",").collect();
                let mut filters: Vec<Filter> = Vec::new();
                for subquery in subqueries.iter() {
                    match Filter::parse(subquery)  {
                        Ok(filter) => filters.push(filter),
                        Err(err) => return Err(err),
                    }
                }
                Ok(Filter::All(filters))
            } else if query.starts_with("not(") && query.ends_with(")") {
                let subquery: &str = &query[5..query.len()-1];
                match Filter::parse(subquery) {
                    Ok(filter) => return Ok(Filter::Not(Box::new(filter))),
                    Err(err) => return Err(err),
                }
            } else if query.starts_with("word=") {
                let value: &str = &query[5..query.len()];
                Ok(Filter::Equals(FieldType::Word, value.to_owned()))
            } else if query.starts_with("phon=") {
                let value: &str = &query[5..query.len()];
                Ok(Filter::Equals(FieldType::Phon, value.to_owned()))
            } else if query.starts_with("example=") {
                let value: &str = &query[8..query.len()];
                Ok(Filter::Equals(FieldType::Example, value.to_owned()))
            } else if query.starts_with("comment=") {
                let value: &str = &query[8..query.len()];
                Ok(Filter::Equals(FieldType::Comment, value.to_owned()))
            } else if query.starts_with("translation=") {
                let value: &str = &query[12..query.len()];
                Ok(Filter::Equals(FieldType::Translation, value.to_owned()))
            } else if query.starts_with("tag=") {
                let value: &str = &query[4..query.len()];
                Ok(Filter::Equals(FieldType::Tag, value.to_owned()))
            } else if query.starts_with("#") { //alias for tags
                let value: &str = &query[1..query.len()];
                Ok(Filter::Equals(FieldType::Tag, value.to_owned()))
            } else {
                Err(SimpleError::new(format!("Unable to parse filter query at: \"{}\"", query)))
            }
        }
    }
}


///we implement the Display trait so we can print VocaCards
impl fmt::Display for VocaCard {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.words.join(" | "))
    }
}

impl VocaCard {
    pub fn compute_id(&mut self) {
        let id_string: String = format!("{}|{}|{}", self.words.join(" / "), self.transcriptions.join(" / "), self.translations.join(" / "));
        let id = md5::compute(id_string.as_bytes());
        self.id = format!("{:x}",id);
    }

    ///Prints a vocaitem
    pub fn print(self: &VocaCard, phon: bool, translation: bool, example: bool) {
        println!("{}", self.words.join(" | "));
        if phon {
            println!("{}", self.transcriptions.join(" | "));
        }
        if example {
            println!("{}", self.examples.join("\n"));
        }
        if translation {
            println!("{}", self.translations.join(" |  "));
        }
    }
}

pub struct CardIterator<'a> {
    pub session: &'a VocaSession,
    pub deck_index: usize, //the selected deck
    pub card_index: usize, //the selected card
    pub multideck: bool,
    pub filter: Filter,
    ///Show all cards, not only the ones that are due
    pub show_all: bool,
}

impl<'a> Iterator for CardIterator<'a> {
    type Item = &'a VocaCard;

    fn next(&mut self) -> Option<Self::Item>  {
        if self.card_index + 1 < self.session.decks[self.deck_index].len()  {
            let card_id = self.session.decks[self.deck_index][self.card_index].as_str();
            self.card_index += 1;
            let card = if let Some(set) = self.session.set.as_ref() {
                set.get(card_id)
            } else {
                None
            };
            if let Some(card) = card {
                if self.filter.matches(card) {
                    if self.show_all || self.session.is_due(card.id.as_str()) {
                        Some(card)
                    } else {
                        self.next() //recurse
                    }
                } else {
                    self.next() //recurse
                }
            } else {
                None
            }
        } else {
            if self.multideck && self.deck_index + 1 < self.session.decks.len() {
                self.deck_index += 1;
                self.next() //recurse
            } else {
                None
            }
        }
    }
}

pub struct CardReverseIterator<'a> {
    pub session: &'a VocaSession,
    pub deck_index: usize, //the selected deck
    pub card_index: usize, //the selected card
    pub multideck: bool,
    pub filter: Filter,
    ///Show all cards, not only the ones that are due
    pub show_all: bool,
}

impl<'a> Iterator for CardReverseIterator<'a> {
    type Item = &'a VocaCard;

    fn next(&mut self) -> Option<Self::Item>  {
        if self.card_index > 0 {
            let card_id = self.session.decks[self.deck_index][self.card_index].as_str();
            self.card_index -= 1;
            let card = if let Some(set) = self.session.set.as_ref() {
                set.get(card_id)
            } else {
                None
            };
            if let Some(card) = card {
                if self.filter.matches(card) {
                    if self.show_all || self.session.is_due(card.id.as_str()) {
                        Some(card)
                    } else {
                        self.next() //recurse
                    }
                } else {
                    self.next() //recurse
                }
            } else {
                None
            }
        } else {
            if self.multideck && self.deck_index > 0 {
                self.deck_index -= 1;
                self.next() //recurse
            } else {
                None
            }
        }
    }
}

impl VocaSet {
    /// Parse the vocabulary data file (JSON or YAML) into the VocaSet structure
    pub fn from_file(filename: &str) -> Result<VocaSet, Box<dyn Error>> {
        let data = fs::read_to_string(filename)?;
        if filename.ends_with(".json") {
            let mut data: VocaSet = serde_json::from_str(data.as_str())?;
            data.filename = filename.to_owned();
            for card in data.cards.iter_mut() {
                card.compute_id();
            }
            Ok(data)
        } else if filename.ends_with(".yml") || filename.ends_with(".yaml") {
            let mut data: VocaSet = serde_yaml::from_str(data.as_str())?;
            data.filename = filename.to_owned();
            for card in data.cards.iter_mut() {
                card.compute_id();
            }
            Ok(data)
        } else {
            bail!("Extension not recognised")
        }
    }

    /*
    /// Add a new item to the vocabulary list
    pub fn append(&mut self, word: String, translation: Option<&str>, transcription: Option<&str>, example: Option<&str>, comment: Option<&str>, tags: Option<&Vec<&str>>) {
        let tags: Vec<String> = if let Some(ref tags) = tags {
            tags.iter()
                .map(|s| { s.to_string() })
                .collect()
        } else {
            Vec::new()
        };
        let item = VocaCard {
            words: vec!(word),
            translations: vec!(translation.map(|s:&str| s.to_string()).unwrap_or(String::new())),
            transcriptions: vec!(transcription.map(|s:&str| s.to_string()).unwrap_or(String::new())),
            example: example.map(|s:&str| s.to_string()).unwrap_or(String::new()),
            comment: comment.map(|s:&str| s.to_string()).unwrap_or(String::new()),
            tags: tags,
        };
        self.items.push(item);
    }*/

    pub fn save_json(&self, filename: &str) -> std::io::Result<()> {
        let data: String = serde_json::to_string(self)?;
        fs::write(filename, data)
    }

    pub fn save_yaml(&self, filename: &str) {
        if let Ok(data) = serde_yaml::to_string(self) {
            fs::write(filename, data);
        }
    }

    /// Show the contents of the Vocabulary Set; prints to to standard output
    pub fn show(&self, withtranslation: bool, withtranscription: bool, filter: &Filter, withtags: bool, withexample: bool, withcomment: bool) {
        for card in self.cards.iter() {
            if filter.matches(card) {
                print!("{}", card);
                if withtranscription { print!("\t{}", card.transcriptions.join(" | ")) }
                if withtranslation { print!("\t{}", card.translations.join(" | ")) }
                if withexample { print!("\t{}", card.examples.join("\n")) }
                if withcomment { print!("\t{}", card.comments.join("\n")) }
                if withtags {
                    print!("\t");
                    for (i, tag) in card.tags.iter().enumerate() {
                        print!("{}", tag);
                        if i < card.tags.len() - 1 {
                            print!(",")
                        }
                    }
                }
                println!()
            }
        }
    }

    ///Output all data as CSV
    pub fn csv(&self, filter: &Filter) -> Result<(), Box<dyn Error>> {
        let mut wtr = csv::WriterBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_writer(io::stdout());
        for card in self.cards.iter() {
            if filter.matches(card) {
                wtr.serialize(card)?;
            }
        };
        wtr.flush()?;
        Ok(())
    }


    pub fn contains(&self, id: &str) -> bool {
        for card in self.cards.iter() {
            if card.id.as_str() == id {
                return true;
            }
        }
        false
    }

    pub fn get(&self, id: &str) -> Option<&VocaCard> {
        for card in self.cards.iter() {
            if card.id.as_str() == id {
                return Some(card);
            }
        }
        None
    }

    ///Lookup a word
    pub fn find(&self, word: &str) -> Option<&VocaCard> {
        self.cards.iter().find(|x| { x.words.contains(&word.to_string()) })
    }
}


impl VocaSession {
    pub fn new(filename: String, set_filename: String, deck_names: Vec<String>) -> Result<VocaSession, Box<dyn Error>> {
        let mut decks: Vec<Vec<String>> = Vec::new();
        for _ in 0..deck_names.len() {
            decks.push(vec!());
        }
        let mut session = VocaSession {
            filename: filename,
            set_filename: set_filename,
            deck_names: deck_names,
            decks: decks,
            correct: HashMap::new(),
            incorrect: HashMap::new(),
            lastvisit: HashMap::new(),
            deck_index: None,
            card_index: None,
            set: None,
            mode: "flashcards".to_string(),
            settings: HashSet::new(),
            settings_int: [
                ("optioncount", 5),
                ("new.interval", 0),
                ("short.interval", 1),
                ("medium.interval", 24),
                ("long.interval", 168),
                ("done.interval", 5040),
            ].iter().map(|(x,y)| (x.to_string(),*y)).collect(),
            settings_str: [
                ("flashcards.front", "word,example"),
                ("flashcards.back", "translation,transcription"),
                ("multiquiz.front", "word,example,options"),
                ("multiquiz.back", "translation,transcription"),
                ("openquiz.front", "word,example"),
                ("openquiz.back", "translation,transcription"),
            ].iter().map(|(x,y)| (x.to_string(),y.to_string())).collect(),
            correct_option: 0,
            options: vec!(),
        };
        session.load_data()?;
        session.populate_decks();
        Ok(session)
    }

    pub fn populate_decks(&mut self) {
        if let Some(set) = &self.set {
            //collects all IDs from all decks
            let mut found = HashSet::new();
            for deck in self.decks.iter_mut() {
                deck.retain( |card_id| set.contains(card_id) ); //remove orphans
                for card_id in deck.iter() {
                    found.insert(card_id.clone());
                }
            }
            //add new cards to first deck
            for card in set.cards.iter() {
                if !found.contains(&card.id) {
                    //append to first deck
                    self.decks[0].push(card.id.clone())
                }
            }
        }
    }

    /// Load session file
    pub fn from_file(filename: &str) -> Result<VocaSession, Box<dyn Error>> {
        let data = fs::read_to_string(filename)?;
        let mut session: VocaSession = serde_json::from_str(data.as_str())?; //(shadowing)
        session.load_data()?;
        session.populate_decks();
        Ok(session)
    }

    pub fn load_data(&mut self) -> Result<&VocaSet, Box<dyn Error>> {
        let set = VocaSet::from_file(self.set_filename.as_str()).map_err(|err| SimpleError::new(format!("Tried to load data {}: {}", self.set_filename.as_str(), err).to_string()))?;
        self.set = Some(set);
        Ok(self.set.as_ref().unwrap())
    }

    ///Save a session file
    pub fn to_file(&self, filename: &str) -> std::io::Result<()> {
        let data: String = serde_json::to_string(self)?;
        fs::write(filename, data)
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.to_file(self.filename.as_str())
    }

    ///Return the 'score' for an item, this corresponds to the probability it is presented if
    ///a deck is sorted by score, and also influences the chance a card is picked as a response
    ///option; the lower the score, the better a word is known
    pub fn score(&self, id: &str) -> f64 {
        let correct = *self.correct.get(id).or(Some(&0)).unwrap() + 1;
        let incorrect = *self.incorrect.get(id).or(Some(&0)).unwrap() + 1;
        incorrect as f64 / correct as f64
    }

    pub fn visit(&mut self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("Unable to get time").as_secs();
        if let Some(card) = self.current_card() {
            self.lastvisit.insert(card.id.to_string(),now);
        }
    }

    pub fn shuffle(&mut self) -> Result<(),SimpleError> {
        if let Some(deck_index) = self.deck_index {
            let mut rng = rand::thread_rng();
            self.decks[deck_index].shuffle(&mut rng);
            Ok(())
        } else {
            Err(SimpleError::new("No deck selected"))
        }
    }

    pub fn select_deck_by_name(&mut self, deck_name: &str) -> Result<(),SimpleError> {
        if let Some(deck_index) = self.deck_names.iter().position(|s| s == deck_name) {
            self.select_deck(deck_index + 1)
        } else {
            Err(SimpleError::new("No deck with that name exists"))
        }
    }

    pub fn select_deck(&mut self, deck_index: usize) -> Result<(),SimpleError> {
        if deck_index > 1 && deck_index < self.deck_names.len() {
            self.deck_index = Some(deck_index - 1);
            self.card_index = None;
            Ok(())
        } else {
            Err(SimpleError::new("Invalid deck"))
        }
    }

    pub fn unselect_deck(&mut self) {
        self.deck_index = None;
    }

    pub fn is_due(&self, id: &str) -> bool {
        if let Some(deck_index) = self.deck_index {
            if let Some(deck_name) = self.deck_names.get(deck_index) {
                let interval: u64 = *self.get_int(format!("{}.interval", deck_name).as_str()).unwrap_or(&0) as u64;
                if interval == 0 {
                    return true;
                } else {
                    if let Some(lastvisit) = self.lastvisit.get(id) {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("Unable to get time").as_secs();
                        return now - lastvisit > interval * 3600;
                    }
                }
            }
        }
        false
    }

    pub fn select_card(&mut self, card_index: usize) -> Result<(),SimpleError> {
        if let Some(deck_index) = self.deck_index {
            if card_index > 0 && card_index < self.decks[deck_index].len() -1 {
                self.card_index = Some(card_index - 1);
                Ok(())
            } else {
                Err(SimpleError::new("Invalid card index"))
            }
        } else {
            Err(SimpleError::new("No deck selected"))
        }
    }

    ///Pick a random card
    pub fn pick(&self) -> Option<&VocaCard> {
        let filter = self.get_filter();
        if let Some(set) = self.set.as_ref() {
            let sum: f64 = set.cards.iter().map(|card| {
                if filter.matches(card) {
                    self.score(card.id.as_str())
                } else {
                    0.0
                }
            }).sum();
            let choice: f64 = rand::random::<f64>() * sum;
            let mut score: f64 = 0.0; //cummulative score
            let mut choiceindex: usize = 0;
            for (i, card) in set.cards.iter().enumerate() {
                if filter.matches(card) {
                    score += self.score(card.id.as_str());
                    if score >= choice {
                        choiceindex = i;
                        break;
                    }
                }
            }
            Some(&set.cards[choiceindex])
        } else {
            None
        }
    }

    pub fn iter(&self) -> CardIterator {
        self.iter_cards(self.deck_index, self.card_index.unwrap_or(0), self.get_filter(), self.settings.contains("showall"))
    }

    ///Iterate over all cards in the currently selected deck
    pub fn iter_cards(&self, deck_index: Option<usize>, card_index: usize, filter: Filter, showall: bool) -> CardIterator {
        if let Some(deck_index) = deck_index {
            CardIterator {
                session: self,
                deck_index: deck_index,
                card_index: card_index,
                multideck: false,
                filter: filter,
                show_all: showall
            }
        } else {
            CardIterator {
                session: self,
                deck_index: 0,
                card_index: 0,
                multideck: true,
                filter: filter,
                show_all: showall
            }
        }
    }

    ///Iterate over all cards in the currently selected deck
    pub fn rev_iter_cards(&self, deck_index: Option<usize>, card_index: usize, filter: Filter, showall: bool) -> CardReverseIterator {
        if let Some(deck_index) = deck_index {
            CardReverseIterator {
                session: self,
                deck_index: deck_index,
                card_index: card_index,
                multideck: false,
                filter: filter,
                show_all: showall
            }
        } else {
            CardReverseIterator {
                session: self,
                deck_index: 0,
                card_index: 0,
                multideck: true,
                filter: filter,
                show_all: showall
            }
        }
    }

    pub fn get_filter(&self) -> Filter {
        let mut filter = Filter::parse(self.get_str("filter").unwrap_or(""));
        if let Err(err) = filter {
            eprintln!("Disabling filter due to parse error: {}",err);
            filter = Ok(Filter::None)
        }
        filter.unwrap()
    }

    ///Promote the card at in the specified deck and card index to the next deck
    ///This corresponds to a correct answer
    pub fn promote(&mut self) -> Result<(), SimpleError> {
        self.visit();
        if let (Some(deck_index), Some(card_index)) = (self.deck_index, self.card_index) {
            if let Some(deck) = self.decks.get_mut(deck_index) {
                let card_id = deck.remove(card_index);
                *self.correct.entry(card_id.clone()).or_insert(0) += 1;
                if let Some(nextdeck) = self.decks.get_mut(deck_index + 1) {
                    nextdeck.push(card_id);
                }
                return Ok(());
            }
        }
        Err(SimpleError::new("Invalid deck or card"))
    }

    ///Demote the card at in the specified deck and card index to the previous deck
    ///This corresponds to an incorrect answer
    pub fn demote(&mut self) -> Result<(), SimpleError> {
        self.visit();
        if let (Some(deck_index), Some(card_index)) = (self.deck_index, self.card_index) {
            if let Some(deck) = self.decks.get_mut(deck_index) {
                let card_id = deck.remove(card_index);
                *self.incorrect.entry(card_id.clone()).or_insert(0) += 1;
                if deck_index > 0 {
                    if let Some(prevdeck) = self.decks.get_mut(deck_index - 1) {
                        prevdeck.push(card_id);
                    }
                }
                return Ok(());
            }
        }
        Err(SimpleError::new("Invalid deck or card"))
    }

    pub fn next_deck(&mut self) -> Result<(), SimpleError> {
        if let Some(deck_index) = self.deck_index.as_mut() {
            if *deck_index < self.decks.len() - 1 {
                *deck_index += 1;
                self.card_index = None;
                self.next_card(false)?;
            } else {
                bail!("No further decks left");
            }
        } else {
            if !self.decks.is_empty() {
                self.deck_index = Some(0);
                self.card_index = None;
                self.next_card(false)?;
            } else {
                bail!("There are no decks at all");
            }
        }
        Ok(())
    }

    pub fn previous_deck(&mut self) -> Result<(), SimpleError> {
        if let Some(deck_index) = self.deck_index.as_mut() {
            if *deck_index > 0 {
                *deck_index -= 1;
                self.card_index = None;
                self.next_card(false)?;
            } else {
                bail!("You are at the first deck");
            }
        } else {
            if !self.decks.is_empty() {
                self.deck_index = Some(0);
                self.card_index = None;
                self.next_card(false)?;
            } else {
                bail!("There are no decks at all");
            }
        }
        Ok(())
    }

    pub fn next_card(&mut self, allow_next_deck: bool) -> Result<(), SimpleError> {
        self.visit();
        let mut iter = self.iter_cards(self.deck_index, self.card_index.unwrap_or(0), self.get_filter(), self.settings.contains("showall"));
        iter.next();
        let deck_index = iter.deck_index.clone();
        let card_index = iter.card_index.clone();
        self.deck_index = Some(deck_index);
        self.card_index = Some(card_index);
        Ok(())
    }

    pub fn previous_card(&mut self) -> Result<(), SimpleError> {
        self.visit();
        let mut iter = self.rev_iter_cards(self.deck_index, self.card_index.unwrap_or(0), self.get_filter(), self.settings.contains("showall"));
        iter.next();
        let deck_index = iter.deck_index.clone();
        let card_index = iter.card_index.clone();
        self.deck_index = Some(deck_index);
        self.card_index = Some(card_index);
        Ok(())
    }

    ///Retrusn the current card
    pub fn current_card(&self) -> Option<&VocaCard> {
        if let (Some(deck_index), Some(card_index)) = (self.deck_index, self.card_index) {
            if let Some(deck) = self.decks.get(deck_index) {
                if let Some(card_id) = deck.get(card_index) {
                    return self.set.as_ref().unwrap().get(card_id);
                }
            }
        }
        None
    }


    pub fn set(&mut self, setting: String) {
        self.settings.insert(setting);
    }

    pub fn unset(&mut self, setting: &str) {
        self.settings.remove(setting);
        self.settings_int.remove(setting);
        self.settings_str.remove(setting);
    }

    pub fn set_int(&mut self, setting: String, value: usize) {
        self.settings_int.insert(setting, value);
    }

    pub fn get_int(&self, setting: &str) -> Option<&usize> {
        self.settings_int.get(setting)
    }

    pub fn set_str(&mut self, setting: String, value: String) {
        self.settings_str.insert(setting, value);
    }

    pub fn get_str(&self, setting: &str) -> Option<&str> {
        self.settings_str.get(setting).map(|s| s.as_str())
    }

    pub fn toggle(&mut self, setting: String) -> bool {
        if self.settings.contains(&setting) {
            self.settings.remove(&setting);
            false
        } else {
            self.settings.insert(setting);
            true
        }
    }

    pub fn card(&self) -> Option<&VocaCard> {
        if let Some(deck_index) = self.deck_index {
            if let Some(card_index) = self.card_index {
                if let Some(set) = self.set.as_ref() {
                    if deck_index >= self.decks.len() {
                        return None;
                    }
                    if card_index >= self.decks[deck_index].len() {
                        return None;
                    }
                    let card_id = &self.decks[deck_index][card_index];
                    return set.get( card_id.as_str() );
                }
            }
        }
        None
    }

    pub fn pick_options(&mut self) {
        let optioncount = self.settings_int.get("optioncount").unwrap_or(&5);
        self.options = Vec::new();
        let mut rng = rand::thread_rng();
        self.correct_option = rng.gen_range(0, *optioncount);
        for i in 0..*optioncount {
            if i == self.correct_option {
                let card = self.current_card().expect("Current card");
                let card_id = card.id.clone();
                self.options.push(card_id);
            } else {
                loop {
                    if let Some(option) = self.pick() {
                        if !self.options.contains(&option.id) {
                            let card_id = option.id.clone();
                            self.options.push(card_id);
                            break;
                        }
                    }
                }
            }
        }

    }
}



/// Return the default data directory
pub fn defaultdatadir() -> PathBuf {
    PathBuf::from(dirs::config_dir().expect("Unable to find configuration dir")).join("vocage").join("data")
}
///
/// Return the default score directory
pub fn defaultsessiondir() -> PathBuf {
    PathBuf::from(dirs::config_dir().expect("Unable to find configuration dir")).join("vocage").join("sessions")
}

pub fn getdatafile(name: &str, datapath: PathBuf) -> PathBuf {
    datapath.join(name.to_owned())
}

pub fn getsessionfile(name: &str, sessionpath: PathBuf) -> PathBuf {
    let mut filename: String = if name.ends_with(".json") {
        name[..name.len()-5].to_string()
    } else {
        name.to_string()
    };
    filename.push_str(".json");
    sessionpath.join(filename)
}

/// Returns an index of available sessions
pub fn getsessionindex(configpath_opt: Option<PathBuf>) -> Vec<String> {
    let mut index: Vec<String> = Vec::new();
    let mut datapath;
    if let Some(configpath_some) = configpath_opt {
        datapath = configpath_some;
    } else {
        datapath = dirs::config_dir().expect("Unable to find configuration dir");
        datapath = PathBuf::from(datapath).join("vocage").join("sessions");
    }
    if datapath.exists() {
        fileindex(datapath, "".to_string(), &mut index);
    }
    index
}

/// Returns an index of available vocabulary sets
pub fn getdataindex(configpath_opt: Option<PathBuf>) -> Vec<String> {
    let mut index: Vec<String> = Vec::new();
    let mut datapath;
    if let Some(configpath_some) = configpath_opt {
        datapath = configpath_some;
    } else {
        datapath = dirs::config_dir().expect("Unable to find configuration dir");
        datapath = PathBuf::from(datapath).join("vocage").join("data");
    }
    if datapath.exists() {
        fileindex(datapath, "".to_string(), &mut index);
    }
    index
}

pub fn fileindex(dir: PathBuf, prefix: String, index: &mut Vec<String>) {
    for file in dir.read_dir().expect("Unable to read directory") {
        if let Ok(file) = file {
            let filename = file.file_name().into_string().unwrap();
            if file.path().is_dir() {
                fileindex(file.path(), format!("{}{}", prefix, filename), index);
            } else {
                index.push(filename);
            }
        }
    }
}

