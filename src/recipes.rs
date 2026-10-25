//! Always two active orders when food is available; each needs three ingredients, with repeats allowed.
use crate::components::{FoodType, Soup};
use bevy::prelude::*;
use rand::{Rng, SeedableRng, rngs::StdRng};

pub const MAX_ACTIVE_ORDERS: usize = 2;

#[derive(Resource)]
pub struct Orders {
    pub active: Vec<Vec<FoodType>>,
    ids: Vec<u64>,
    next_id: u64,
    available: Vec<FoodType>,
    rng: StdRng,
}
impl Orders {
    pub fn new(available: Vec<FoodType>) -> Self {
        Self::with_seed(available, rand::random())
    }
    fn with_seed(mut available: Vec<FoodType>, seed: u64) -> Self {
        available.sort();
        available.dedup();
        let mut orders = Self {
            active: Vec::new(),
            ids: Vec::new(),
            next_id: 0,
            available,
            rng: StdRng::seed_from_u64(seed),
        };
        for _ in 0..MAX_ACTIVE_ORDERS {
            orders.add_order();
        }
        orders
    }
    fn add_order(&mut self) {
        if self.available.is_empty() || self.active.len() >= MAX_ACTIVE_ORDERS {
            return;
        }
        let mut recipe = (0..crate::constants::MAX_INGREDIENTS)
            .map(|_| self.available[self.rng.random_range(0..self.available.len())])
            .collect::<Vec<_>>();
        recipe.sort();
        self.active.push(recipe);
        self.ids.push(self.next_id);
        self.next_id += 1;
    }
    pub fn order_id(&self, index: usize) -> Option<u64> {
        self.ids.get(index).copied()
    }
    pub fn matching_index(&self, soup: &Soup) -> Option<usize> {
        if !soup.is_prepared() {
            return None;
        }
        let mut actual = soup.ingredients.clone();
        actual.sort();
        self.active.iter().position(|recipe| *recipe == actual)
    }
    pub fn complete(&mut self, index: usize) {
        self.active.remove(index);
        self.ids.remove(index);
        self.add_order();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recipes_allow_repeats_but_only_use_real_crates() {
        let orders = Orders::with_seed(vec![FoodType::Fish, FoodType::Fish], 1);
        assert_eq!(orders.active, vec![vec![FoodType::Fish; 3]; 2]);
        let available = vec![FoodType::Tomato, FoodType::Fish, FoodType::LotusRoot];
        let mut orders = Orders::with_seed(available.clone(), 7);
        let mut combinations = std::collections::HashSet::new();
        for _ in 0..100 {
            for recipe in &orders.active {
                assert_eq!(recipe.len(), 3);
                assert!(recipe.iter().all(|food| available.contains(food)));
                combinations.insert(recipe.clone());
            }
            orders.complete(0);
            orders.complete(1);
        }
        assert!(combinations.len() > 1);
        assert!(
            combinations
                .iter()
                .any(|recipe| recipe.contains(&FoodType::LotusRoot))
        );
        assert!(Orders::with_seed(vec![], 1).active.is_empty());
    }
    #[test]
    fn completion_refills_immediately_and_preserves_the_other_order() {
        let mut orders = Orders::with_seed(vec![FoodType::Tomato, FoodType::Fish], 9);
        for index in [1, 0, 1, 0] {
            let retained = orders.active[1 - index].clone();
            orders.complete(index);
            assert_eq!(orders.active.len(), 2);
            assert_eq!(orders.active[0], retained);
            assert_eq!(orders.active[1].len(), 3);
        }
        let mut identical = Orders::with_seed(vec![FoodType::Fish], 2);
        for _ in 0..10 {
            identical.complete(0);
            assert_eq!(identical.active, vec![vec![FoodType::Fish; 3]; 2]);
        }
    }
    #[test]
    fn either_order_can_be_delivered_and_counts_cooking_and_burning_are_checked() {
        let mut orders = Orders::with_seed(vec![FoodType::Tomato, FoodType::Fish], 1);
        orders.active = vec![
            vec![FoodType::Fish; 3],
            vec![FoodType::Tomato, FoodType::Tomato, FoodType::Fish],
        ];
        let mut soup = Soup::new(FoodType::Fish);
        soup.ingredients
            .extend([FoodType::Tomato, FoodType::Tomato]);
        assert_eq!(orders.matching_index(&soup), None);
        soup.cook_time_elapsed = 24.0;
        soup.refresh_cook_state();
        assert_eq!(orders.matching_index(&soup), Some(1));
        orders.complete(1);
        assert_eq!(orders.active.len(), 2);
        assert_eq!(orders.active[0], vec![FoodType::Fish; 3]);
        soup.ingredients = vec![FoodType::Fish; 3];
        assert_eq!(orders.matching_index(&soup), Some(0));
        soup.state = crate::components::CookState::Burnt;
        assert_eq!(orders.matching_index(&soup), None);
    }
}
