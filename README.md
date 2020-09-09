# Introduction

Vocage is a simple terminal-based vocabulary-learning tool. It presents flashcards using a spaced-repetition algorithm
(e.g. Leitner); words you know well will be presented less and less frequently and words you have problems with will be
repeated more often. You quickly move cards/words between decks and each deck has an associated presentation interval.

You can use vocage for anything you'd use flashcards for and not necessarily limited
to learning languages.

The aim of this software is to keep things simple and minimalistic and to focus on one thing only (the unix philosphy).

## Features & non-features

* Data is stored in a simple plain-text tab-separated values (TSV) format. So you can edit your cards in your favourite
  text editor or spreadsheet program. Vocage itself does not provide editing facilities.
* Progress is stored right inside the TSV files (added columns for the deck a card is on and when it is due). This keeps
  everything in one place. You could keep your vocabulary sets in git, if you want.
* Configuration is done via command line parameters that can also be stored as comments in the TSV file for quick loading:
    * You determine what columns to show on which side of the card. Traditionally there's a front
        side and a back side, but you can define as many sides as you want.
    * Deck/interval configuration can be passed as command line parameters, and is stored as comments in the TSV file.
    * Sane defaults; if no configuration is specified some sane defaults will be used
* Can load multiple vocabulary files (TSV) at once if they have the same column layout. This allows you
  to use files as an easy grouping mechanism (e.g. a file per level, per domain, or per word class).
* The 'fancy' TUI can be disabled by setting the ``--minimal`` parameter, in case you want to interact with vocage
  from shell scripts or other software.
* Colour support, each column gets a colour (can be disabled in ``--minimal`` mode)
* Arrow keys and vim-style movements (hjkl)
* Written in Rust; fast & efficient

## Installation

Install it using Rust's package manager:

```
cargo install sesdiff
```

No cargo/rust on your system yet? Do ``sudo apt install cargo`` on Debian/ubuntu based systems, ``brew install rust`` on mac, or use [rustup](https://rustup.rs/).


## Usage

### Quick Start

Have some data in TSV format ready, for example [from here](https://github.com/proycon/vocadata):

```
$ vocage yourdata.tsv
```

### Key Bindings

* space / enter - 'Flip' the card, shows the next side (i.e. the solution)
* Arrow down / ``j`` - Skip this card for now and go to the next card (a random card will be selected)
* Arrow up / ``k`` - Skip this card for now and go to the previous card
* Arrow right / ``l`` - Promote this card to the next deck
* Arrow left / ``h`` - Promote this card to the previous deck
* A number key - Move the card to the n'th deck
* ``w`` - Save progress (input files will be amended)
* ``q`` - Quit, doesn't do any saving.





