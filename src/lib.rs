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
    pub mode: VocaMode,
    #[serde(default)]
    pub deck_index: Option<usize>, //the selected deck
    #[serde(default)]
    pub card_index: Option<usize>, //the selected card
    #[serde(default)]
    pub settings: HashSet<String>,
    #[serde(default)]
    pub settings_int: HashMap<String, usize>,
    #[serde(skip)]
    pub set: Option<VocaSet>,
}

#[derive(Serialize, Deserialize,Clone,Copy,Debug)]
pub enum VocaMode {
    None,
    Flashcards,
    OpenQuiz,
    ChoiceQuiz,
    MatchQuiz,
}

impl Default for VocaMode {
    fn default() -> VocaMode {
        VocaMode::None
    }
}

impl VocaMode {
    pub fn from_str(s: &str) -> Result<VocaMode,SimpleError> {
        match s.to_lowercase().as_str() {
            "none" => Ok(Self::None),
            "flashcards" => Ok(Self::Flashcards),
            "openquiz" => Ok(Self::OpenQuiz),
            "choicequiz" => Ok(Self::ChoiceQuiz),
            _ => Err(SimpleError::new("not a valid mode"))
        }
    }
}

impl fmt::Display for VocaMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{:?}",self)
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

    pub fn filter(&self, filtertags: Option<&Vec<&str>>) -> bool {
        match filtertags {
            Some(tags) => match tags.is_empty() {
               false => {
                   //do the actual matching
                   self.tags.iter().any(|tag| tags.contains(&tag.as_str()))
               },
               true => true
            },
            None => true
        }
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
}

impl<'a> Iterator for CardIterator<'a> {
    type Item = &'a VocaCard;

    fn next(&mut self) -> Option<Self::Item>  {
        if self.card_index + 1< self.session.decks[self.deck_index].len()  {
            let card_id = self.session.decks[self.deck_index][self.card_index].as_str();
            self.card_index += 1;
            let card = if let Some(set) = self.session.set.as_ref() {
                set.get(card_id)
            } else {
                None
            };
            card
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
    pub fn show(&self, withtranslation: bool, withtranscription: bool, filtertags: Option<&Vec<&str>>, withtags: bool, withexample: bool, withcomment: bool) {
        for card in self.cards.iter() {
            if card.filter(filtertags) {
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
    pub fn csv(&self, filtertags: Option<&Vec<&str>>) -> Result<(), Box<dyn Error>> {
        let mut wtr = csv::WriterBuilder::new()
            .flexible(true)
            .has_headers(false)
            .from_writer(io::stdout());
        for card in self.cards.iter() {
            if card.filter(filtertags) {
                wtr.serialize(card)?;
            }
        };
        wtr.flush()?;
        Ok(())
    }

    ///Select a word
    /*
    pub fn pick(&self, deck, session: &mut VocaSession, filtertags: Option<&Vec<&str>>, visit: bool) -> &VocaCard {
        let sum: f64 = self.items.iter().map(|item| {
            if item.filter(filtertags) {
                session.score(item.id_as_string().as_str())
            } else {
                0.0
            }
        }).sum();
        let choice: f64 = rand::random::<f64>() * sum;
        let mut score: f64 = 0.0; //cummulative score
        let mut choiceindex: usize = 0;
        for (i, item) in self.items.iter().enumerate() {
            if item.filter(filtertags) {
                if let Some(ref scoredata) = session {
                    score += scoredata.score(item.id_as_string().as_str());
                } else {
                    score += 1.0;
                }
                if score >= choice {
                    choiceindex = i;
                    break;
                }
            }
        }
        let vocaitem = &self.items[choiceindex];
        if visit {
            if let Some(ref mut scoredata) = session {
                scoredata.visit(vocaitem);
            }
        }
        vocaitem
    }
    */

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
            mode: VocaMode::None,
            settings: HashSet::new(),
            settings_int: HashMap::new(),
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

    ///Return the 'score' for an item, this corresponds to the probability it is presented, so
    ///the lower the score, the better a word is known
    pub fn score(&self, id: &str) -> f64 {
        let correct = *self.correct.get(id).or(Some(&0)).unwrap() + 1;
        let incorrect = *self.incorrect.get(id).or(Some(&0)).unwrap() + 1;
        incorrect as f64 / correct as f64
    }

    pub fn visit(&mut self, item_id: &str) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("Unable to get time").as_secs();
        self.lastvisit.insert(item_id.to_owned(),now);
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

    ///Iterate over all cards in the currently selected deck
    pub fn iter(&self) -> CardIterator {
        if let Some(deck_index) = self.deck_index {
            CardIterator {
                session: self,
                deck_index: deck_index,
                card_index: 0,
                multideck: false,
            }
        } else {
            CardIterator {
                session: self,
                deck_index: 0,
                card_index: 0,
                multideck: true,
            }
        }
    }

    ///Promote the card at in the specified deck and card index to the next deck
    ///This corresponds to a correct answer
    pub fn promote(&mut self) -> Result<(), SimpleError> {
        if let (Some(deck_index), Some(card_index)) = (self.deck_index, self.card_index) {
            if let Some(deck) = self.decks.get_mut(deck_index) {
                let card_id = deck.remove(card_index);
                self.visit(card_id.as_str());
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
        if let (Some(deck_index), Some(card_index)) = (self.deck_index, self.card_index) {
            if let Some(deck) = self.decks.get_mut(deck_index) {
                let card_id = deck.remove(card_index);
                self.visit(card_id.as_str());
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
            if *deck_index < self.decks.len() {
                *deck_index += 1;
            } else {
                bail!("No further decks left");
            }
        } else {
            if !self.decks.is_empty() {
                self.deck_index = Some(0);
                self.card_index = None;
                self.next_card()?;
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
            } else {
                bail!("You are at the first deck");
            }
        } else {
            if !self.decks.is_empty() {
                self.deck_index = Some(0);
                self.card_index = None;
                self.next_card()?;
            } else {
                bail!("There are no decks at all");
            }
        }
        Ok(())
    }

    pub fn next_card(&mut self) -> Result<(), SimpleError> {
        if let Some(deck_index) = self.deck_index {
            if let Some(card_index) = self.card_index.as_mut() {
                if *card_index < self.decks[deck_index].len() {
                    *card_index += 1;
                } else {
                    self.next_deck()?;  //will recurse back into this function if needed
                }
            } else {
                if self.decks[deck_index].is_empty() {
                    eprintln!("Deck #{} is empty", deck_index + 1);
                    self.next_deck()?;  //will recurse back into this function if needed
                } else {
                    self.card_index = Some(0)
                }
            }
        } else {
            self.next_deck()?;  //will recurse back into this function if needed
        }
        Ok(())
    }

    pub fn previous_card(&mut self) -> Result<(), SimpleError> {
        if let Some(deck_index) = self.deck_index {
            if let Some(card_index) = self.card_index.as_mut() {
                if *card_index > 0 {
                    *card_index -= 1;
                } else {
                    self.previous_deck()?;  //will recurse back into this function if needed
                }
            } else {
                if self.decks[deck_index].is_empty() {
                    eprintln!("Deck #{} is empty", deck_index + 1);
                    self.previous_deck()?;  //will recurse back into this function if needed
                } else {
                    self.card_index = Some(self.decks[deck_index].len())
                }
            }
        } else {
            self.previous_deck()?;  //will recurse back into this function if needed
        }
        Ok(())
    }


    pub fn set(&mut self, setting: String) {
        self.settings.insert(setting);
    }

    pub fn unset(&mut self, setting: &str) {
        self.settings.remove(setting);
    }

    pub fn set_int(&mut self, setting: String, value: usize) {
        self.settings_int.insert(setting, value);
    }

    pub fn get_int(&mut self, setting: &str) -> Option<&usize> {
        self.settings_int.get(setting)
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
                    let card_id = &self.decks[deck_index][card_index];
                    return set.get( card_id.as_str() );
                }
            }
        }
        None
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
        for file in datapath.read_dir().expect("Unable to read dir") {
            if let Ok(file) = file {
                index.push(file.file_name().into_string().unwrap());
            }
        }
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
        for file in datapath.read_dir().expect("Unable to read dir") {
            if let Ok(file) = file {
                index.push(file.file_name().into_string().unwrap());
            }
        }
    }
    index
}

