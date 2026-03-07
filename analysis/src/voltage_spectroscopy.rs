//! Voltage spectroscopy analysis.
use argmin::core::State;
use std::iter;

type Vector1 = nalgebra::DVector<f64>;
type Matrix2 = nalgebra::DMatrix<f64>;

struct Exp<'a> {
    x: &'a Vec<f64>,
    y: &'a Vec<f64>,
}

impl<'a> argmin::core::Operator for Exp<'a> {
    type Param = Vector1;
    type Output = Vector1;

    fn apply(&self, param: &Self::Param) -> Result<Self::Output, argmin_math::Error> {
        let a = param[0];
        let b = param[1];
        let c = param[2];

        let residuals = iter::zip(self.x.iter(), self.y.iter())
            .map(|(x, y)| y - (a * (b * x).exp() + c))
            .collect();
        Ok(Vector1::from_vec(residuals))
    }
}

impl<'a> argmin::core::Jacobian for Exp<'a> {
    type Param = Vector1;
    type Jacobian = Matrix2;

    fn jacobian(&self, param: &Self::Param) -> Result<Self::Jacobian, argmin_math::Error> {
        let a = param[0];
        let b = param[1];

        let jac = Matrix2::from_fn(self.x.len(), 3, |i, j| {
            let x = self.x[i];
            let exp = (b * x).exp();
            match j {
                0 => -exp,
                1 => -a * x * exp,
                2 => -1.0,
                _ => unreachable!("invalid index"),
            }
        });

        Ok(jac)
    }
}

pub fn exponential(x: &Vec<f64>, y: &Vec<f64>) -> Result<(f64, f64, f64), Error> {
    if x.len() != y.len() {
        return Err(Error::InvalidData);
    }
    if x.len() == 0 {
        return Err(Error::NoData);
    }
    let c = y.iter().min_by(|a, b| a.total_cmp(b)).unwrap();
    let y_max = y.iter().max_by(|a, b| a.total_cmp(b)).unwrap();
    let a = y_max - c;
    let b = 1.0;
    let p0 = Vector1::from_vec(vec![a, b, *c]);
    let problem = Exp { x, y };
    let solver = argmin::solver::gaussnewton::GaussNewton::<f64>::new();

    let res = argmin::core::Executor::new(problem, solver)
        .configure(|state| state.param(p0).max_iters(100))
        .run()?;

    let Some(best) = res.state().get_best_param() else {
        return Err(Error::NoConvergence);
    };

    Ok((best[0], best[1], best[2]))
}

#[derive(Debug, derive_more::From)]
pub enum Error {
    InvalidData,
    NoData,
    Optimization(argmin::core::Error),
    NoConvergence,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_exponential() {
        let a = 2.0;
        let b: f64 = 2.0;
        let c = 2.0;
        let x = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let y = x.iter().map(|x| a * (b * x).exp() + c).collect();
        let (ap, bp, cp) = exponential(&x, &y).unwrap();

        approx::assert_relative_eq!(a, ap, epsilon = 10.0 * f64::EPSILON);
        approx::assert_relative_eq!(b, bp, epsilon = 10.0 * f64::EPSILON);
        approx::assert_relative_eq!(c, cp, epsilon = 10.0 * f64::EPSILON);
    }
}
