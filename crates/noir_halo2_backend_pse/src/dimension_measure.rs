// copied and adapted from https://github.com/privacy-scaling-explorations/halo2wrong/blob/master/halo2wrong/src/utils.rs
// cus fn k() is private.

use pse_halo2wrong::{
    curves::group::ff::PrimeField,
    halo2::{
        circuit::Value,
        plonk::{
            Advice, Any, Assigned, Assignment, Challenge, Circuit, Column, ConstraintSystem, Error,
            Fixed, FloorPlanner, Instance, Selector,
        },
    },
};
use std::{cell::RefCell, ops::RangeInclusive};

#[derive(Default)]
pub struct DimensionMeasurement {
    instance: RefCell<u64>,
    advice: RefCell<u64>,
    fixed: RefCell<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dimension {
    blinding_factor: u64,
    instance: u64,
    advice: u64,
    fixed: u64,
}

impl Dimension {
    pub(crate) fn k(&self) -> u32 {
        u64::BITS
            - ([self.instance, self.advice, self.fixed]
                .into_iter()
                .max_by(Ord::cmp)
                .expect("Unexpected empty column iterator")
                + self.blinding_factor)
                .next_power_of_two()
                .leading_zeros()
            - 1
    }

    pub(crate) fn instance(&self) -> u64 {
        self.instance
    }

    pub(crate) fn advice(&self) -> u64 {
        self.advice
    }

    pub(crate) fn fixed(&self) -> u64 {
        self.fixed
    }

    fn _advice_range(&self) -> RangeInclusive<usize> {
        0..=self.advice as usize
    }
}

impl DimensionMeasurement {
    fn update<C: Into<Any>>(&self, column: C, offset: usize) {
        let mut target = match column.into() {
            Any::Instance => self.instance.borrow_mut(),
            Any::Advice(_advice) => self.advice.borrow_mut(),
            Any::Fixed => self.fixed.borrow_mut(),
        };
        if offset as u64 > *target {
            *target = offset as u64;
        }
    }

    pub fn measure<F: PrimeField, C: Circuit<F>>(circuit: &C) -> Result<Dimension, Error> {
        let mut cs = ConstraintSystem::default();
        let config = C::configure(&mut cs);
        let mut measurement = Self::default();
        C::FloorPlanner::synthesize(&mut measurement, circuit, config, cs.constants().to_vec())?;
        Ok(Dimension {
            blinding_factor: cs.blinding_factors() as u64,
            instance: measurement.instance.take(),
            advice: measurement.advice.take(),
            fixed: measurement.fixed.take(),
        })
    }
}

impl<F: PrimeField> Assignment<F> for DimensionMeasurement {
    fn enter_region<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
    }

    fn exit_region(&mut self) {}

    fn get_challenge(&self, _challenge: Challenge) -> Value<F> {
        Value::unknown()
    }

    fn enable_selector<A, AR>(&mut self, _: A, _: &Selector, offset: usize) -> Result<(), Error>
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        self.update(Fixed, offset);
        Ok(())
    }

    fn query_instance(&self, _: Column<Instance>, offset: usize) -> Result<Value<F>, Error> {
        self.update(Instance, offset);
        Ok(Value::unknown())
    }

    fn annotate_column<A, AR>(&mut self, _annotation: A, _column: Column<Any>)
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        // Do nothing.
    }

    fn assign_advice<V, VR, A, AR>(
        &mut self,
        _: A,
        _: Column<Advice>,
        offset: usize,
        _: V,
    ) -> Result<(), Error>
    where
        V: FnOnce() -> Value<VR>,
        VR: Into<Assigned<F>>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        self.update(Any::advice(), offset);
        Ok(())
    }

    fn assign_fixed<V, VR, A, AR>(
        &mut self,
        _: A,
        _: Column<Fixed>,
        offset: usize,
        _: V,
    ) -> Result<(), Error>
    where
        V: FnOnce() -> Value<VR>,
        VR: Into<Assigned<F>>,
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        self.update(Fixed, offset);
        Ok(())
    }

    fn copy(
        &mut self,
        lhs: Column<Any>,
        offset_lhs: usize,
        rhs: Column<Any>,
        offset_rhs: usize,
    ) -> Result<(), Error> {
        self.update(*lhs.column_type(), offset_lhs);
        self.update(*rhs.column_type(), offset_rhs);
        Ok(())
    }

    fn fill_from_row(
        &mut self,
        _: Column<Fixed>,
        offset: usize,
        _: Value<Assigned<F>>,
    ) -> Result<(), Error> {
        self.update(Fixed, offset);
        Ok(())
    }

    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
    }

    fn pop_namespace(&mut self, _: Option<String>) {}
}
