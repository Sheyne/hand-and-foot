#![feature(drain_filter)]

use std::collections::HashMap;

use itertools::iproduct;
use rand::seq::SliceRandom;
use rand::thread_rng;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Copy, Clone, PartialEq, Eq, Debug, EnumIter, Hash)]
enum Rank {
    Ace,
    Two,
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
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, EnumIter)]
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Card {
    Regular(Rank, Suit),
    Joker,
}

impl Card {
    pub fn iter() -> impl Iterator<Item = Self> {
        iproduct!(Rank::iter(), Suit::iter())
            .map(|(rank, suit)| Card::Regular(rank, suit))
            .chain([Card::Joker, Card::Joker])
    }

    pub fn points(self) -> usize {
        match self {
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
    pub fn play(&mut self, rank: Rank, mut cards: Vec<Card>) -> Result<(), TurnError> {
        if !cards.iter().all(|c| {
            c.is_wild()
                || match c {
                    Card::Regular(r, _) => *r == rank,
                    _ => unreachable!("We filtered above"),
                }
        }) {
            return Err(TurnError::NotAllCardsMatchRank);
        }

        let mut cards_have = vec![];
        for card in &self.hand {
            if let Some(position) = cards.iter().position(|c| *c == *card) {
                cards.remove(position);
                cards_have.push(*card);
            }
        }
        if !cards.is_empty() {
            return Err(TurnError::NotAllCardsInHand);
        }
        let already_played = self.play_area.entry(rank).or_insert(vec![]);
        let num_wild = already_played
            .iter()
            .chain(cards_have.iter())
            .filter(|x| x.is_wild())
            .count();
        let total_num = already_played.len() + cards_have.len();

        if total_num >= 7 {
            return Err(TurnError::TooManyCardsInBook);
        }
        if num_wild >= (total_num - num_wild) {
            return Err(TurnError::TooManyWildsInBook);
        }

        for card in cards_have.iter() {
            self.hand
                .remove(self.hand.iter().position(|c| *c == *card).unwrap());
        }

        if self.hand.len() == 0 && self.foot.is_some()
            || self.foot.is_none() && self.hand.len() <= 1
        {
            if let Some(foot) = self.foot.take() {
                self.hand = foot;
            } else {
                self.hand.extend(cards_have);
                return Err(TurnError::MustKeepOneCardInHand);
            }
        }

        already_played.extend(cards_have.drain(..));

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

    pub fn add(&mut self, card: Card) {
        self.0.push(card)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, EnumIter)]
enum DrawAction {
    Pickup,
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
    TooManyWildsInBook,
    NotEnoughCards,
}

#[derive(Clone, Debug)]
struct Game {
    players: Vec<PlayerCards>,
    deck: Deck,
    discard_pile: Deck,
}

impl Game {
    pub fn deal(num_players: usize) -> Self {
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
            deck,
            discard_pile: Deck::empty(),
        }
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

    fn pickup(&mut self, player_idx: usize) -> Result<(), TurnError> {
        let player = &mut self.players[player_idx];

        for card in self.discard_pile.take(7).ok_or(TurnError::NotEnoughCards)? {
            player.hand.push(card);
        }

        Ok(())
    }

    pub fn take_turn<DA, PA, RA>(
        &mut self,
        player_idx: usize,
        draw_action: DA,
        play_action: PA,
        discard_action: RA,
    ) -> Result<TurnResult, TurnError>
    where
        DA: FnOnce(&PlayerCards) -> DrawAction,
        PA: FnOnce(&mut PlayerCards),
        RA: Fn(&PlayerCards) -> Card,
    {
        self.resolve_red_threes(player_idx)?;

        match draw_action(&self.players[player_idx]) {
            DrawAction::Pickup => self.pickup(player_idx).or_else(|_| self.draw(player_idx)),
            DrawAction::Draw => self.draw(player_idx),
        }?;

        self.resolve_red_threes(player_idx)?;

        play_action(&mut self.players[player_idx]);

        loop {
            let discard = discard_action(&self.players[player_idx]);
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
    let mut game = Game::deal(4);

    game.take_turn(
        0,
        |_player| DrawAction::Draw,
        |player| {
            dbg!(player);
        },
        |player| *player.hand.first().unwrap(),
    )
    .unwrap();

    dbg!(game);
}
