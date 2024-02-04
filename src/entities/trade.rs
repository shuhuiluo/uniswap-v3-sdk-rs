use crate::prelude::*;
use anyhow::Result;
use std::{cmp::Ordering, collections::HashSet};
use uniswap_sdk_core::{constants::TradeType, prelude::*};

/// Trades comparator, an extension of the input output comparator that also considers other dimensions of the trade in ranking them
///
/// ## Arguments
///
/// * `a`: The first trade to compare
/// * `b`: The second trade to compare
///
pub fn trade_comparator<TInput: CurrencyTrait, TOutput: CurrencyTrait, P: Clone>(
    a: &mut Trade<TInput, TOutput, P>,
    b: &mut Trade<TInput, TOutput, P>,
) -> Ordering {
    // must have same input and output token for comparison
    assert!(
        a.swaps[0]
            .input_amount
            .meta
            .currency
            .equals(&b.swaps[0].input_amount.meta.currency),
        "INPUT_CURRENCY"
    );
    assert!(
        a.swaps[0]
            .output_amount
            .meta
            .currency
            .equals(&b.swaps[0].output_amount.meta.currency),
        "OUTPUT_CURRENCY"
    );
    let a_input = a.input_amount().unwrap().as_fraction();
    let b_input = b.input_amount().unwrap().as_fraction();
    let a_output = a.output_amount().unwrap().as_fraction();
    let b_output = b.output_amount().unwrap().as_fraction();
    if a_output == b_output {
        if a_input == b_input {
            // consider the number of hops since each hop costs gas
            let a_hops = a
                .swaps
                .iter()
                .map(|s| s.route.token_path.len())
                .sum::<usize>();
            let b_hops = b
                .swaps
                .iter()
                .map(|s| s.route.token_path.len())
                .sum::<usize>();
            return a_hops.cmp(&b_hops);
        }
        // trade A requires less input than trade B, so A should come first
        if a_input < b_input {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    } else {
        // tradeA has less output than trade B, so should come second
        if a_output < b_output {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct BestTradeOptions {
    /// how many results to return
    pub max_num_results: Option<usize>,
    /// the maximum number of hops a trade should contain
    pub max_hops: Option<usize>,
}

/// Represents a swap through a route
#[derive(Clone, PartialEq, Debug)]
pub struct Swap<TInput: CurrencyTrait, TOutput: CurrencyTrait, P> {
    pub route: Route<TInput, TOutput, P>,
    pub input_amount: CurrencyAmount<TInput>,
    pub output_amount: CurrencyAmount<TOutput>,
}

/// Represents a trade executed against a set of routes where some percentage of the input is split across each route.
///
/// Each route has its own set of pools. Pools can not be re-used across routes.
///
/// Does not account for slippage, i.e., changes in price environment that can occur between the time the trade is
/// submitted and when it is executed.
#[derive(Clone, PartialEq, Debug)]
pub struct Trade<TInput: CurrencyTrait, TOutput: CurrencyTrait, P> {
    /// The swaps of the trade, i.e. which routes and how much is swapped in each that make up the trade.
    pub swaps: Vec<Swap<TInput, TOutput, P>>,
    /// The type of the trade, either exact in or exact out.
    pub trade_type: TradeType,
    /// The cached result of the input amount computation
    _input_amount: Option<CurrencyAmount<TInput>>,
    /// The cached result of the output amount computation
    _output_amount: Option<CurrencyAmount<TOutput>>,
    /// The cached result of the computed execution price
    _execution_price: Option<Price<TInput, TOutput>>,
    /// The cached result of the price impact computation
    _price_impact: Option<Percent>,
}

impl<TInput, TOutput, T, P> Trade<TInput, TOutput, P>
where
    TInput: CurrencyTrait,
    TOutput: CurrencyTrait,
    T: TickTrait,
    P: TickDataProvider<Tick = T>,
{
    /// Construct a trade by passing in the pre-computed property values
    ///
    /// ## Arguments
    ///
    /// * `swaps`: The routes through which the trade occurs
    /// * `trade_type`: The type of trade, exact input or exact output
    ///
    fn new(swaps: Vec<Swap<TInput, TOutput, P>>, trade_type: TradeType) -> Result<Self> {
        let input_currency = &swaps[0].input_amount.meta.currency.wrapped();
        let output_currency = &swaps[0].output_amount.meta.currency.wrapped();
        for Swap { route, .. } in &swaps {
            assert!(
                input_currency.equals(&route.input.wrapped()),
                "INPUT_CURRENCY_MATCH"
            );
            assert!(
                output_currency.equals(&route.output.wrapped()),
                "OUTPUT_CURRENCY_MATCH"
            );
        }
        let num_pools = swaps
            .iter()
            .map(|swap| swap.route.pools.len())
            .sum::<usize>();
        let mut pool_address_set = HashSet::<Address>::new();
        for Swap { route, .. } in &swaps {
            for pool in &route.pools {
                pool_address_set.insert(pool.address(None, None));
            }
        }
        assert_eq!(num_pools, pool_address_set.len(), "POOLS_DUPLICATED");
        Ok(Trade {
            swaps,
            trade_type,
            _input_amount: None,
            _output_amount: None,
            _execution_price: None,
            _price_impact: None,
        })
    }

    /// Constructs an exact in trade with the given amount in and route
    ///
    /// ## Arguments
    ///
    /// * `route`: The route of the exact in trade
    /// * `amount_in`: The amount being passed in
    ///
    pub fn exact_in(
        route: Route<TInput, TOutput, P>,
        amount_in: CurrencyAmount<Token>,
    ) -> Result<Self> {
        Self::from_route(route, amount_in, TradeType::ExactInput)
    }

    /// Constructs an exact out trade with the given amount out and route
    ///
    /// ## Arguments
    ///
    /// * `route`: The route of the exact out trade
    /// * `amount_out`: The amount returned by the trade
    ///
    pub fn exact_out(
        route: Route<TInput, TOutput, P>,
        amount_out: CurrencyAmount<Token>,
    ) -> Result<Self> {
        Self::from_route(route, amount_out, TradeType::ExactOutput)
    }

    /// Constructs a trade by simulating swaps through the given route
    ///
    /// ## Arguments
    ///
    /// * `route`: The route to swap through
    /// * `amount`: The amount specified, either input or output, depending on `trade_type`
    /// * `trade_type`: Whether the trade is an exact input or exact output swap
    ///
    pub fn from_route(
        route: Route<TInput, TOutput, P>,
        amount: CurrencyAmount<Token>,
        trade_type: TradeType,
    ) -> Result<Self> {
        let length = route.token_path.len();
        let mut amounts: Vec<CurrencyAmount<Token>> =
            vec![CurrencyAmount::from_raw_amount(route.input.wrapped(), 0,)?; length];
        let input_amount: CurrencyAmount<TInput>;
        let output_amount: CurrencyAmount<TOutput>;
        match trade_type {
            TradeType::ExactInput => {
                assert!(amount.meta.currency.equals(&route.input), "INPUT");
                amounts[0] = amount.wrapped()?;
                for i in 0..length - 1 {
                    let pool = &route.pools[i];
                    let (output_amount, _) = pool.get_output_amount(amounts[i].clone(), None)?;
                    amounts[i + 1] = output_amount;
                }
                input_amount = CurrencyAmount::from_fractional_amount(
                    route.input.clone(),
                    amount.numerator(),
                    amount.denominator(),
                )?;
                output_amount = CurrencyAmount::from_fractional_amount(
                    route.output.clone(),
                    amounts[length - 1].numerator(),
                    amounts[length - 1].denominator(),
                )?;
            }
            TradeType::ExactOutput => {
                assert!(amount.meta.currency.equals(&route.output), "OUTPUT");
                amounts[length - 1] = amount.wrapped()?;
                for i in (1..=length - 1).rev() {
                    let pool = &route.pools[i];
                    let (input_amount, _) = pool.get_input_amount(amounts[i].clone(), None)?;
                    amounts[i - 1] = input_amount;
                }
                input_amount = CurrencyAmount::from_fractional_amount(
                    route.input.clone(),
                    amounts[0].numerator(),
                    amounts[0].denominator(),
                )?;
                output_amount = CurrencyAmount::from_fractional_amount(
                    route.output.clone(),
                    amount.numerator(),
                    amount.denominator(),
                )?;
            }
        }
        Trade::new(
            vec![Swap {
                route,
                input_amount,
                output_amount,
            }],
            trade_type,
        )
    }

    /// Constructs a trade from routes by simulating swaps
    ///
    /// ## Arguments
    ///
    /// * `routes`: The routes to swap through and how much of the amount should be routed through each
    /// * `trade_type`: Whether the trade is an exact input or exact output swap
    ///
    pub fn from_routes(
        routes: Vec<(CurrencyAmount<Token>, Route<TInput, TOutput, P>)>,
        trade_type: TradeType,
    ) -> Result<Self> {
        let mut populated_routes: Vec<Swap<TInput, TOutput, P>> = Vec::with_capacity(routes.len());
        for (amount, route) in routes {
            let trade = Self::from_route(route, amount, trade_type)?;
            populated_routes.push(trade.swaps[0].clone());
        }
        Trade::new(populated_routes, trade_type)
    }

    /// Creates a trade without computing the result of swapping through the route.
    /// Useful when you have simulated the trade elsewhere and do not have any tick data
    pub fn create_unchecked_trade(
        route: Route<TInput, TOutput, P>,
        input_amount: CurrencyAmount<TInput>,
        output_amount: CurrencyAmount<TOutput>,
        trade_type: TradeType,
    ) -> Result<Self> {
        Trade::new(
            vec![Swap {
                route,
                input_amount,
                output_amount,
            }],
            trade_type,
        )
    }

    /// Creates a trade without computing the result of swapping through the routes.
    /// Useful when you have simulated the trade elsewhere and do not have any tick data
    pub fn create_unchecked_trade_with_multiple_routes(
        swaps: Vec<Swap<TInput, TOutput, P>>,
        trade_type: TradeType,
    ) -> Result<Self> {
        Trade::new(swaps, trade_type)
    }
}

impl<TInput, TOutput, P> Trade<TInput, TOutput, P>
where
    TInput: CurrencyTrait,
    TOutput: CurrencyTrait,
    P: Clone,
{
    /// When the trade consists of just a single route, this returns the route of the trade.
    pub fn route(&self) -> Route<TInput, TOutput, P> {
        assert_eq!(self.swaps.len(), 1, "MULTIPLE_ROUTES");
        self.swaps[0].route.clone()
    }

    /// The input amount for the trade assuming no slippage.
    pub fn input_amount(&mut self) -> Result<CurrencyAmount<TInput>> {
        if let Some(ref input_amount) = self._input_amount {
            return Ok(input_amount.clone());
        }
        let mut total =
            CurrencyAmount::from_raw_amount(self.swaps[0].input_amount.meta.currency.clone(), 0)?;
        for Swap { input_amount, .. } in &self.swaps {
            total = total.add(input_amount)?;
        }
        self._input_amount = Some(total.clone());
        Ok(total)
    }

    /// The output amount for the trade assuming no slippage.
    pub fn output_amount(&mut self) -> Result<CurrencyAmount<TOutput>> {
        if let Some(ref output_amount) = self._output_amount {
            return Ok(output_amount.clone());
        }
        let mut total =
            CurrencyAmount::from_raw_amount(self.swaps[0].output_amount.meta.currency.clone(), 0)?;
        for Swap { output_amount, .. } in &self.swaps {
            total = total.add(output_amount)?;
        }
        self._output_amount = Some(total.clone());
        Ok(total)
    }

    /// The price expressed in terms of output amount/input amount.
    pub fn execution_price(&mut self) -> Result<Price<TInput, TOutput>> {
        if let Some(ref execution_price) = self._execution_price {
            return Ok(execution_price.clone());
        }
        let input_amount = self.input_amount()?;
        let output_amount = self.output_amount()?;
        let execution_price = Price::new(
            input_amount.meta.currency.clone(),
            output_amount.meta.currency.clone(),
            input_amount.quotient(),
            output_amount.quotient(),
        );
        self._execution_price = Some(execution_price.clone());
        Ok(execution_price)
    }

    /// Returns the percent difference between the route's mid price and the price impact
    pub fn price_impact(&mut self) -> Result<Percent> {
        if let Some(ref price_impact) = self._price_impact {
            return Ok(price_impact.clone());
        }
        let mut spot_output_amount =
            CurrencyAmount::from_raw_amount(self.output_amount()?.meta.currency, 0)?;
        for Swap {
            route,
            input_amount,
            ..
        } in &mut self.swaps
        {
            let mid_price = route.mid_price()?;
            spot_output_amount = spot_output_amount.add(&mid_price.quote(input_amount.clone())?)?;
        }
        let price_impact = spot_output_amount
            .subtract(&self.output_amount()?)?
            .divide(&spot_output_amount)?;
        self._price_impact = Some(Percent::new(
            price_impact.numerator(),
            price_impact.denominator(),
        ));
        Ok(self._price_impact.clone().unwrap())
    }

    /// Get the minimum amount that must be received from this trade for the given slippage tolerance
    ///
    /// ## Arguments
    ///
    /// * `slippage_tolerance`: The tolerance of unfavorable slippage from the execution price of this trade
    ///
    pub fn minimum_amount_out(
        &mut self,
        slippage_tolerance: Percent,
    ) -> Result<CurrencyAmount<TOutput>> {
        assert!(
            slippage_tolerance >= Percent::new(0, 1),
            "SLIPPAGE_TOLERANCE"
        );
        let output_amount = self.output_amount()?;
        if self.trade_type == TradeType::ExactOutput {
            return self.output_amount();
        }
        let slippage_adjusted_amount_out = ((Percent::new(1, 1) + slippage_tolerance).invert()
            * Percent::new(output_amount.quotient(), 1))
        .quotient();
        CurrencyAmount::from_raw_amount(
            output_amount.meta.currency.clone(),
            slippage_adjusted_amount_out,
        )
        .map_err(|e| e.into())
    }

    /// Get the maximum amount in that can be spent via this trade for the given slippage tolerance
    ///
    /// ## Arguments
    ///
    /// * `slippage_tolerance`: The tolerance of unfavorable slippage from the execution price of this trade
    /// * `amount_in`: The amount to spend
    ///
    pub fn maximum_amount_in(
        &mut self,
        slippage_tolerance: Percent,
        amount_in: Option<CurrencyAmount<TInput>>,
    ) -> Result<CurrencyAmount<TInput>> {
        assert!(
            slippage_tolerance >= Percent::new(0, 1),
            "SLIPPAGE_TOLERANCE"
        );
        let amount_in = amount_in.unwrap_or(self.input_amount()?);
        if self.trade_type == TradeType::ExactInput {
            return Ok(amount_in);
        }
        let slippage_adjusted_amount_in = ((Percent::new(1, 1) + slippage_tolerance)
            * Percent::new(amount_in.quotient(), 1))
        .quotient();
        CurrencyAmount::from_raw_amount(
            amount_in.meta.currency.clone(),
            slippage_adjusted_amount_in,
        )
        .map_err(|e| e.into())
    }

    /// Return the execution price after accounting for slippage tolerance
    ///
    /// ## Arguments
    ///
    /// * `slippage_tolerance`: The allowed tolerated slippage
    ///
    pub fn worst_execution_price(
        &mut self,
        slippage_tolerance: Percent,
    ) -> Result<Price<TInput, TOutput>> {
        Ok(Price::new(
            self.input_amount()?.meta.currency.clone(),
            self.output_amount()?.meta.currency.clone(),
            self.maximum_amount_in(slippage_tolerance.clone(), None)?
                .quotient(),
            self.minimum_amount_out(slippage_tolerance)?.quotient(),
        ))
    }
}
