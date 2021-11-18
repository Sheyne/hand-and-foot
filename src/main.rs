#![feature(drain_filter)]
#![feature(exact_size_is_empty)]

use itertools::{iproduct, Itertools};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashMap;
use std::fmt::Display;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Copy, Clone, PartialEq, Eq, Debug, EnumIter, Hash, PartialOrd, Ord)]
enum Round {
    One,
    Two,
    Three,
    Four,
}

impl Round {
    fn meld(self) -> usize {
        match self {
            Round::One => 90,
            Round::Two => 120,
            Round::Three => 150,
            Round::Four => 180,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, EnumIter, Hash, PartialOrd, Ord)]
enum Rank {
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
    Two,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, EnumIter, PartialOrd, Ord)]
enum Suit {
    Diamond,
    Club,
    Heart,
    Spade,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, EnumIter)]
enum Color {
    Red,
    Black,
}

impl Display for Suit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Suit::Diamond => "♦️",
            Suit::Club => "♣️",
            Suit::Heart => "♥️",
            Suit::Spade => "♠️",
        })
    }
}

impl Display for Rank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Rank::Ace => "A",
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
        })
    }
}

impl Suit {
    pub fn color(self) -> Color {
        match self {
            Self::Heart | Self::Diamond => Color::Red,
            Self::Spade | Self::Club => Color::Black,
        }
    }
}

impl Rank {
    pub fn points(self) -> usize {
        match self {
            Self::Ace | Self::Two => 20,
            Self::Three => 0,
            Self::Four | Self::Five | Self::Six | Self::Seven => 5,
            Self::Eight | Self::Nine | Self::Ten | Self::Jack | Self::Queen | Self::King => 10,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
enum Card {
    Regular(Rank, Suit),
    Joker,
}

impl Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Card::Regular(r, s) => f.write_fmt(format_args!("{}{}", r, s)),
            Card::Joker => f.write_str("🃏"),
        }
    }
}

impl Card {
    pub fn iter() -> impl Iterator<Item = Self> {
        iproduct!(Rank::iter(), Suit::iter())
            .map(|(rank, suit)| Card::Regular(rank, suit))
            .chain([Card::Joker, Card::Joker])
    }

    pub fn points(self) -> usize {
        match self {
            Card::Regular(Rank::Three, s) if s.color() == Color::Red => 100,
            Card::Regular(rank, _) => rank.points(),
            Card::Joker => 50,
        }
    }

    pub fn is_wild(self) -> bool {
        match self {
            Card::Joker | Card::Regular(Rank::Two, _) => true,
            _ => false,
        }
    }

    pub fn can_be_booked(self) -> bool {
        match self {
            Self::Regular(Rank::Three | Rank::Two, _) | Self::Joker => false,
            _ => true,
        }
    }

    pub fn rank(self) -> Option<Rank> {
        match self {
            Card::Regular(r, _) => Some(r),
            Card::Joker => None,
        }
    }
}

#[derive(Clone, Debug)]
struct PlayerCards {
    hand: Vec<Card>,
    foot: Option<Vec<Card>>,
    books: Vec<Vec<Card>>,
    red_threes: usize,
    play_area: HashMap<Rank, Vec<Card>>,
}

impl PlayerCards {
    pub fn can_play(
        &self,
        round: Round,
        cards: &HashMap<Rank, Vec<Card>>,
    ) -> Result<(), TurnError> {
        let has_melded = self.play_area.values().flatten().any(|_| true);
        if !has_melded {
            if cards
                .iter()
                .flat_map(|(_, g)| g.iter())
                .map(|c| c.points())
                .sum::<usize>()
                < round.meld()
            {
                return Err(TurnError::NotEnoughMeld);
            }
        }

        for (rank, cards) in cards.iter() {
            self.can_play_rank(*rank, cards)?;
        }

        Ok(())
    }

    pub fn play(
        &mut self,
        round: Round,
        cards: &HashMap<Rank, Vec<Card>>,
    ) -> Result<(), TurnError> {
        self.can_play(round, cards)?;

        for (rank, cards) in cards.iter() {
            self.play_rank(*rank, cards)?;
        }

        Ok(())
    }

    pub fn can_play_rank(&self, rank: Rank, cards: &[Card]) -> Result<(), TurnError> {
        if !cards.iter().all(|c| {
            c.is_wild()
                || match c {
                    Card::Regular(r, _) => *r == rank,
                    _ => unreachable!("We filtered above"),
                }
        }) {
            return Err(TurnError::NotAllCardsMatchRank);
        }

        let mut cards_left_to_check = cards.to_owned();
        for card in &self.hand {
            if let Some(position) = cards_left_to_check.iter().position(|c| *c == *card) {
                cards_left_to_check.remove(position);
            }
        }
        if !cards_left_to_check.is_empty() {
            return Err(TurnError::NotAllCardsInHand);
        }
        let already_played = self.play_area.get(&rank);
        let num_wild = already_played
            .iter()
            .flat_map(|x| x.iter())
            .chain(cards.iter())
            .filter(|x| x.is_wild())
            .count();
        let total_num = already_played.map(|x| x.len()).unwrap_or(0) + cards.len();

        if total_num > 7 {
            return Err(TurnError::TooManyCardsInBook);
        }
        if total_num < 3 {
            return Err(TurnError::TooFewCardsInBook);
        }
        if num_wild >= (total_num - num_wild) {
            return Err(TurnError::TooManyWildsInBook);
        }

        Ok(())
    }

    pub fn clean_books(&self) -> usize {
        self.books
            .iter()
            .filter(|book| book.iter().all(|c| !c.is_wild()) && book[0].rank() != Some(Rank::Seven))
            .count()
    }

    pub fn dirty_books(&self) -> usize {
        self.books
            .iter()
            .filter(|book| !book.iter().all(|c| !c.is_wild()))
            .count()
    }

    pub fn seven_books(&self) -> usize {
        self.books
            .iter()
            .filter(|book| book.iter().all(|c| !c.is_wild()) && book[0].rank() == Some(Rank::Seven))
            .count()
    }

    pub fn can_go_out(&self) -> bool {
        self.clean_books() >= 1 && self.dirty_books() >= 1
    }

    pub fn play_rank(&mut self, rank: Rank, cards: &[Card]) -> Result<(), TurnError> {
        self.can_play_rank(rank, cards)?;
        // todo: need to detect if we would be able to go out after playing (not currently implemented)
        // also need to actually undo the play if we fail with can't go out. this check should
        // really be in the can_play_rank function
        let can_go_out = self.can_go_out();
        let already_played = self.play_area.entry(rank).or_insert(vec![]);

        for card in cards.iter() {
            self.hand
                .remove(self.hand.iter().position(|c| *c == *card).unwrap());
        }

        if self.hand.len() == 0 && self.foot.is_some()
            || self.foot.is_none() && self.hand.len() <= 1
        {
            if let Some(foot) = self.foot.take() {
                self.hand = foot;
            } else {
                if !can_go_out || self.hand.len() == 0 {
                    self.hand.extend(cards);
                    return Err(TurnError::MustKeepOneCardInHand);
                }
            }
        }

        already_played.extend(cards);

        if already_played.len() == 7 {
            self.books.push(already_played.drain(..).collect());
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct Deck(Vec<Card>);

impl Deck {
    pub fn empty() -> Self {
        Self(vec![])
    }

    pub fn deal(num_players: usize) -> Self {
        let mut deck: Vec<_> = (0..(num_players + 1))
            .into_iter()
            .flat_map(|_| Card::iter())
            .collect();

        deck.shuffle(&mut thread_rng());

        Self(deck)
    }

    pub fn take(&mut self, num: usize) -> Option<Vec<Card>> {
        if self.0.len() >= num {
            Some(
                (0..num)
                    .into_iter()
                    .map(|_| self.0.pop().unwrap())
                    .collect(),
            )
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn push(&mut self, card: Card) {
        self.0.push(card)
    }
}

#[derive(Clone, PartialEq, Eq, Debug, EnumIter)]
enum DrawAction {
    Pickup(HashMap<Rank, Vec<Card>>),
    Draw,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum TurnResult {
    Over,
    Out,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum TurnError {
    NotAllCardsMatchRank,
    MustKeepOneCardInHand,
    NotAllCardsInHand,
    TooManyCardsInBook,
    TooFewCardsInBook,
    TooManyWildsInBook,
    NotEnoughMeld,
    CanOnlyPickupBookable,
    NotEnoughCards,
    DeckIsLockedNeedTwoInHand,
}

#[derive(Clone, Debug)]
struct Game {
    players: Vec<PlayerCards>,
    round: Round,
    deck: Deck,
    locked: bool,
    discard_pile: Deck,
}

macro_rules! undo_on_error {
    ($y:block $x:block) => {{
        let mut try_block = || $x;

        let err = try_block();
        if err.is_err() {
            $y
        }
        err
    }};
}

impl Game {
    pub fn deal(round: Round, num_players: usize) -> Self {
        let mut deck = Deck::deal(num_players);

        let players = (0..num_players)
            .into_iter()
            .map(|_| PlayerCards {
                hand: deck.take(11).unwrap(),
                foot: Some(deck.take(11).unwrap()),
                books: vec![],
                red_threes: 0,
                play_area: HashMap::new(),
            })
            .collect();

        Self {
            players,
            round,
            deck,
            locked: false,
            discard_pile: Deck::empty(),
        }
    }

    pub fn score(&self) -> Vec<isize> {
        self.players
            .iter()
            .map(|player| {
                let base: isize = player.clean_books() as isize * 500
                    + player.dirty_books() as isize * 300
                    + player.seven_books() as isize * 1500;

                let count: isize = player
                    .books
                    .iter()
                    .flat_map(|x| x.iter())
                    .chain(player.play_area.iter().flat_map(|(_, cs)| cs.iter()))
                    .map(|x| x.points() as isize)
                    .sum();

                let hand_points: isize = player
                    .hand
                    .iter()
                    .chain(player.foot.iter().flatten())
                    .map(|c| c.points() as isize)
                    .sum();

                (player.red_threes as isize) * 100 + base + count - hand_points
            })
            .collect()
    }

    fn resolve_red_threes(&mut self, player_idx: usize) -> Result<(), TurnError> {
        let player = &mut self.players[player_idx];

        let red_threes_in_hand = player
            .hand
            .drain_filter(
                |c| matches!(c, Card::Regular(Rank::Three, suit) if suit.color() == Color::Red),
            )
            .count();

        if red_threes_in_hand == 0 {
            Ok(())
        } else {
            player.red_threes += red_threes_in_hand;

            for card in self
                .deck
                .take(red_threes_in_hand)
                .ok_or(TurnError::NotEnoughCards)?
            {
                player.hand.push(card);
            }

            self.resolve_red_threes(player_idx)
        }
    }

    fn draw(&mut self, player_idx: usize) -> Result<(), TurnError> {
        let player = &mut self.players[player_idx];

        for card in self.deck.take(2).ok_or(TurnError::NotEnoughCards)? {
            player.hand.push(card);
        }

        Ok(())
    }

    fn pickup(
        &mut self,
        player_idx: usize,
        mut cards_to_play: HashMap<Rank, Vec<Card>>,
    ) -> Result<(), TurnError> {
        let player = &mut self.players[player_idx];

        if self.discard_pile.len() < 7 {
            return Err(TurnError::NotEnoughCards);
        }

        let top_card = self.discard_pile.take(1).expect("Checked 7 cards above")[0];

        undo_on_error!( { self.discard_pile.push(top_card); }
        {
            if !top_card.can_be_booked() {
                return Err(TurnError::CanOnlyPickupBookable);
            }

            if self.locked {
                if player
                    .hand
                    .iter()
                    .filter(|x| x.rank() == top_card.rank())
                    .count()
                    < 2
                {
                    return Err(TurnError::DeckIsLockedNeedTwoInHand);
                }
            }

            player.hand.push(top_card);

            undo_on_error!({ player.hand.pop(); }
            {
                let stack = cards_to_play
                .entry(top_card.rank().unwrap())
                .or_insert_with(|| vec![]);
                stack.push(top_card);

                player.can_play(self.round, &cards_to_play)?;
                player.play(self.round, &cards_to_play)?;

                let pickup = self.discard_pile.take(6).expect("Checked 7 cards earlier");
                for card in pickup {
                    player.hand.push(card);
                }

                Ok(())
            })
        })
    }

    pub fn take_turn<DA, PA, RA>(
        &mut self,
        player_idx: usize,
        draw_action: DA,
        play_action: PA,
        discard_action: RA,
    ) -> Result<TurnResult, TurnError>
    where
        DA: FnOnce(Round, &PlayerCards) -> DrawAction,
        PA: FnOnce(Round, &mut PlayerCards),
        RA: Fn(Round, &PlayerCards) -> Card,
    {
        self.resolve_red_threes(player_idx)?;

        match draw_action(self.round, &self.players[player_idx]) {
            DrawAction::Pickup(to_play) => self
                .pickup(player_idx, to_play)
                .or_else(|_| self.draw(player_idx)),
            DrawAction::Draw => self.draw(player_idx),
        }?;

        self.resolve_red_threes(player_idx)?;

        play_action(self.round, &mut self.players[player_idx]);

        loop {
            let discard = discard_action(self.round, &self.players[player_idx]);
            let player = &mut self.players[player_idx];
            if let Some(position) = player.hand.iter().position(|c| *c == discard) {
                player.hand.remove(position);
                break;
            }
        }

        if self.players[player_idx].hand.len() == 0 {
            if let Some(foot) = self.players[player_idx].foot.take() {
                self.players[player_idx].hand = foot;
            } else {
                return Ok(TurnResult::Out);
            }
        }

        Ok(TurnResult::Over)
    }
}

fn main() {
    let mut game = Game::deal(Round::One, 4);

    for _ in 0..50 {
        game.take_turn(
            0,
            |_, _player| DrawAction::Draw,
            |round, player| {
                let mut cards = player.play_area.clone();
                for card in &player.hand {
                    if let Some(rank) = card.rank() {
                        if card.can_be_booked() {
                            cards.entry(rank).or_insert_with(|| vec![]).push(*card);
                        }
                    }
                }

                for wild in player.hand.iter().filter(|c| c.is_wild()) {
                    if let Some(stack) = cards.values_mut().find(|stack| stack.len() == 2) {
                        stack.push(*wild);
                    }
                }

                let to_remove: Vec<_> = cards
                    .iter()
                    .filter(|(_, cs)| cs.len() < 3)
                    .map(|(r, _)| *r)
                    .collect();
                for rank in to_remove {
                    cards.remove(&rank);
                }

                for (rank, existing_cards) in &player.play_area {
                    if let Some(to_add) = cards.get_mut(&rank) {
                        for card in existing_cards {
                            to_add.remove(to_add.iter().position(|x| x == card).unwrap());
                        }
                        while existing_cards.len() + to_add.len() > 7 {
                            to_add.pop();
                        }
                    }
                }

                let num_attempting_to_play: usize = cards.values().map(Vec::len).sum();
                if player.foot.is_none() {
                    if num_attempting_to_play == player.hand.len() {
                        dbg!("Could go out!");
                    }
                }

                if !cards.keys().is_empty() {
                    let err = player.play(round, &cards);
                    if err.is_err() {
                        dbg!(err);
                    }
                }

                // println!(
                //     "{}",
                //     player
                //         .hand
                //         .iter()
                //         .sorted()
                //         .map(ToString::to_string)
                //         .join(", ")
                // );
            },
            |_, player| *player.hand.first().unwrap(),
        )
        .unwrap();
    }

    dbg!(game.score());

    println!("{}", game.players[0].hand[0]);
}
