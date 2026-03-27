#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror,
    Env, Address, String, panic_with_error,
};

// ------------------- TYPES -------------------

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admin: Address,
    pub market_count: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct Market {
    pub id: u64,
    pub question: String,
    pub creator: Address,
    pub created_at: u64,
    pub deadline: u64,
    pub yes_pool: i128,
    pub no_pool: i128,
    pub resolved: bool,
    pub outcome: Option<bool>,
}

#[contracttype]
#[derive(Clone)]
pub struct UserPosition {
    pub yes_amount: i128,
    pub no_amount: i128,
    pub claimed: bool,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    Market(u64),
    Position(u64, Address),
}

// ------------------- ERRORS -------------------

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    MarketNotFound = 3,
    MarketExpired = 4,
    MarketResolved = 5,
    NotAuthorized = 6,
    AlreadyClaimed = 7,
    InvalidAmount = 8,
}

// ------------------- CONTRACT -------------------

#[contract]
pub struct PredictionContract;

#[contractimpl]
impl PredictionContract {

    // -------- INIT --------

    pub fn init(env: Env, admin: Address) {
        let key = DataKey::Config;

        if env.storage().persistent().has(&key) {
            panic_with_error!(&env, Error::AlreadyInitialized);
        }

        let config = Config {
            admin,
            market_count: 0,
        };

        env.storage().persistent().set(&key, &config);
    }

    // -------- CREATE MARKET --------

    pub fn create_market(
        env: Env,
        creator: Address,
        question: String,
        deadline: u64,
    ) -> u64 {
        let mut config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));

        let id = config.market_count;
        config.market_count += 1;

        let market = Market {
            id,
            question,
            creator,
            created_at: env.ledger().timestamp(),
            deadline,
            yes_pool: 0,
            no_pool: 0,
            resolved: false,
            outcome: None,
        };

        env.storage().persistent().set(&DataKey::Market(id), &market);
        env.storage().persistent().set(&DataKey::Config, &config);

        id
    }

    // -------- PLACE BET --------

    pub fn place_bet(
        env: Env,
        user: Address,
        market_id: u64,
        amount: i128,
        bet_yes: bool,
    ) {
        if amount <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let mut market: Market = env
            .storage()
            .persistent()
            .get(&DataKey::Market(market_id))
            .unwrap_or_else(|| panic_with_error!(&env, Error::MarketNotFound));

        if market.resolved {
            panic_with_error!(&env, Error::MarketResolved);
        }

        if env.ledger().timestamp() > market.deadline {
            panic_with_error!(&env, Error::MarketExpired);
        }

        if bet_yes {
            market.yes_pool += amount;
        } else {
            market.no_pool += amount;
        }

        let key = DataKey::Position(market_id, user.clone());

        let mut position: UserPosition = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(UserPosition {
                yes_amount: 0,
                no_amount: 0,
                claimed: false,
            });

        if bet_yes {
            position.yes_amount += amount;
        } else {
            position.no_amount += amount;
        }

        env.storage().persistent().set(&key, &position);
        env.storage().persistent().set(&DataKey::Market(market_id), &market);
    }

    // -------- RESOLVE MARKET --------

    pub fn resolve_market(
        env: Env,
        admin: Address,
        market_id: u64,
        outcome: bool,
    ) {
        let config: Config = env
            .storage()
            .persistent()
            .get(&DataKey::Config)
            .unwrap_or_else(|| panic_with_error!(&env, Error::NotInitialized));

        if admin != config.admin {
            panic_with_error!(&env, Error::NotAuthorized);
        }

        let mut market: Market = env
            .storage()
            .persistent()
            .get(&DataKey::Market(market_id))
            .unwrap_or_else(|| panic_with_error!(&env, Error::MarketNotFound));

        if market.resolved {
            panic_with_error!(&env, Error::MarketResolved);
        }

        market.resolved = true;
        market.outcome = Some(outcome);

        env.storage().persistent().set(&DataKey::Market(market_id), &market);
    }

    // -------- CLAIM --------

    pub fn claim(env: Env, user: Address, market_id: u64) {
        let market: Market = env
            .storage()
            .persistent()
            .get(&DataKey::Market(market_id))
            .unwrap_or_else(|| panic_with_error!(&env, Error::MarketNotFound));

        if !market.resolved {
            panic_with_error!(&env, Error::MarketResolved);
        }

        let key = DataKey::Position(market_id, user.clone());

        let mut position: UserPosition = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| panic_with_error!(&env, Error::InvalidAmount));

        if position.claimed {
            panic_with_error!(&env, Error::AlreadyClaimed);
        }

        let outcome = market.outcome.unwrap();

        let user_bet = if outcome {
            position.yes_amount
        } else {
            position.no_amount
        };

        if user_bet == 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }

        let winner_pool = if outcome {
            market.yes_pool
        } else {
            market.no_pool
        };

        let loser_pool = if outcome {
            market.no_pool
        } else {
            market.yes_pool
        };

        let payout = user_bet + (user_bet * loser_pool / winner_pool);

        // TODO: token transfer in next phase

        position.claimed = true;

        env.storage().persistent().set(&key, &position);
    }
}
