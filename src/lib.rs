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

/// Vocabulary Item data structure
#[derive(Serialize, Deserialize)]
pub struct VocaItem {
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
    pub items: Vec<VocaItem>
}

#[derive(Serialize, Deserialize)]
pub struct VocaSession {
    ///Filename of the session
    pub filename: String,
    ///Filename of the vocabulary set
    pub set_filename: String,
    ///Deck names
    pub deck_names: Vec<String>,
    ///mapping of deck index to vocaitem IDs
    pub decks: Vec<Vec<String>>,
    ///Number of times answered correctly (i.e. moved to the next deck)
    pub correct: HashMap<String,u32>,
    ///Number of times answered incorrectly (i.e. moved to the previous deck()
    pub incorrect: HashMap<String,u32>,
    ///Last presentation by random pick method
    pub lastvisit: HashMap<String,u64>,
}

///we implement the Display trait so we can print VocaItems
impl fmt::Display for VocaItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.words.join(" / "))
    }
}

impl VocaItem {
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
    pub fn print(self: &VocaItem, phon: bool, translation: bool, example: bool) {
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

impl VocaSet {
    /// Parse the vocabulary data file (JSON or YAML) into the VocaSet structure
    pub fn parse(filename: &str) -> Result<VocaSet, Box<dyn Error>> {
        let data = fs::read_to_string(filename)?;
        if filename.ends_with(".json") {
            let mut data: VocaSet = serde_json::from_str(data.as_str())?;
            data.filename = filename.to_owned();
            for item in data.items.iter_mut() {
                item.compute_id();
            }
            Ok(data)
        } else if filename.ends_with(".yml") || filename.ends_with(".yaml") {
            let mut data: VocaSet = serde_yaml::from_str(data.as_str())?;
            data.filename = filename.to_owned();
            for item in data.items.iter_mut() {
                item.compute_id();
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
        let item = VocaItem {
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

    /// Show the contents of the Vocabulary List; prints to to standard output
    pub fn show(&self, withtranslation: bool, withtranscription: bool, filtertags: Option<&Vec<&str>>, withtags: bool, withexample: bool, withcomment: bool) {
        for item in self.items.iter() {
            if item.filter(filtertags) {
                print!("{}", item);
                if withtranscription { print!("\t{}", item.transcriptions.join(" | ")) }
                if withtranslation { print!("\t{}", item.translations.join(" | ")) }
                if withexample { print!("\t{}", item.examples.join("\n")) }
                if withcomment { print!("\t{}", item.comments.join("\n")) }
                if withtags {
                    print!("\t");
                    for (i, tag) in item.tags.iter().enumerate() {
                        print!("{}", tag);
                        if i < item.tags.len() - 1 {
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
        for item in self.items.iter() {
            if item.filter(filtertags) {
                wtr.serialize(item)?;
            }
        };
        wtr.flush()?;
        Ok(())
    }

    ///Select a word
    /*
    pub fn pick(&self, deck, session: &mut VocaSession, filtertags: Option<&Vec<&str>>, visit: bool) -> &VocaItem {
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
        for item in self.items.iter() {
            if item.id.as_str() == id {
                return true;
            }
        }
        false
    }

    pub fn get(&self, id: &str) -> Option<&VocaItem> {
        for item in self.items.iter() {
            if item.id.as_str() == id {
                return Some(item);
            }
        }
        None
    }

    ///Lookup a word
    pub fn find(&self, word: &str) -> Option<&VocaItem> {
        self.items.iter().find(|x| { x.words.contains(&word.to_string()) })
    }
}


impl VocaSession {
    pub fn new(filename: String, set: &VocaSet, deck_names: Vec<String>) -> VocaSession {
        let mut decks: Vec<Vec<String>> = Vec::new();
        for _ in 0..deck_names.len() {
            decks.push(vec!());
        }
        let mut session = VocaSession {
            filename: filename,
            set_filename: set.filename.clone(),
            deck_names: deck_names,
            decks: decks,
            correct: HashMap::new(),
            incorrect: HashMap::new(),
            lastvisit: HashMap::new()
        };
        session.populate_decks(set);
        session
    }

    pub fn populate_decks(&mut self, set: &VocaSet) {
        //collects all IDs from all decks
        let mut found = HashSet::new();
        for deck in self.decks.iter_mut() {
            deck.retain( |item_id| set.contains(item_id) ); //remove orphans
            for item_id in deck.iter() {
                found.insert(item_id.clone());
            }
        }
        //add new items to first deck
        for item in set.items.iter() {
            if !found.contains(&item.id) {
                //append to first deck
                self.decks[0].push(item.id.clone())
            }
        }
    }

    /// Load session file
    pub fn load(filename: &str) -> Result<VocaSession, Box<dyn Error>> {
        let data = fs::read_to_string(filename)?;
        let data: VocaSession = serde_json::from_str(data.as_str())?; //(shadowing)
        Ok(data)
    }

    ///Save a session file
    pub fn save(&self, filename: &str) -> std::io::Result<()> {
        let data: String = serde_json::to_string(self)?;
        fs::write(filename, data)
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

    //shuffle a deck randomly
    pub fn shuffle(&mut self, deck_index: usize) {
        let mut rng = rand::thread_rng();
        if deck_index < self.decks.len() {
            self.decks[deck_index].shuffle(&mut rng);
        }
    }

    ///Promote the item at in the specified deck and item index to the next deck
    ///This corresponds to a correct answer
    pub fn promote(&mut self, deck_index: usize, item_index: usize) {
        if let Some(deck) = self.decks.get_mut(deck_index) {
            if let item_id = deck.remove(item_index) {
                self.visit(item_id.as_str());
                *self.correct.entry(item_id.clone()).or_insert(0) += 1;
                if let Some(nextdeck) = self.decks.get_mut(deck_index + 1) {
                    nextdeck.push(item_id);
                }
            }
        }
    }

    ///Demote the item at in the specified deck and item index to the previous deck
    ///This corresponds to an incorrect answer
    pub fn demote(&mut self, deck_index: usize, item_index: usize) {
        if let Some(deck) = self.decks.get_mut(deck_index) {
            if let item_id = deck.remove(item_index) {
                self.visit(item_id.as_str());
                *self.incorrect.entry(item_id.clone()).or_insert(0) += 1;
                if let Some(prevdeck) = self.decks.get_mut(deck_index - 1) {
                    prevdeck.push(item_id);
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

pub fn getdatafile(name: &str, datapath: PathBuf) -> Option<PathBuf> {
    let datafile = datapath.join(name.to_owned());
    match datafile.exists() {
        true => Some(datafile),
        false => None
    }
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
pub fn getsessionindex(configpath_opt: Option<PathBuf>) -> Vec<PathBuf> {
    let mut index: Vec<PathBuf> = Vec::new();
    let configpath;
    if let Some(configpath_some) = configpath_opt {
        configpath = configpath_some;
    } else {
        configpath = dirs::config_dir().expect("Unable to find configuration dir");
    }
    let datapath = PathBuf::from(configpath).join("vocage").join("sessions");
    if datapath.exists() {
        for file in datapath.read_dir().expect("Unable to read dir") {
            if let Ok(file) = file {
                index.push(file.path());
            }
        }
    }
    index
}

/// Returns an index of available vocabulary sets
pub fn getdataindex(configpath_opt: Option<PathBuf>) -> Vec<PathBuf> {
    let mut index: Vec<PathBuf> = Vec::new();
    let configpath;
    if let Some(configpath_some) = configpath_opt {
        configpath = configpath_some;
    } else {
        configpath = dirs::config_dir().expect("Unable to find configuration dir");
    }
    let datapath = PathBuf::from(configpath).join("vocage").join("data");
    if datapath.exists() {
        for file in datapath.read_dir().expect("Unable to read dir") {
            if let Ok(file) = file {
                index.push(file.path());
            }
        }
    }
    index
}

