/// An easing curve that maps linear progress `t` (0.0..=1.0) to eased progress.
///
/// ```
/// use chromakopia::animate::Easing;
///
/// let t = 0.5;
/// assert_eq!(Easing::Linear.apply(t), 0.5);
/// assert!(Easing::EaseIn.apply(t) < 0.5);  // slow start
/// assert!(Easing::EaseOut.apply(t) > 0.5);  // slow end
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub enum Easing {
    /// No easing — constant speed.
    #[default]
    Linear,
    /// Cubic ease-in — slow start, fast end.
    EaseIn,
    /// Cubic ease-out — fast start, slow end.
    EaseOut,
    /// Cubic ease-in-out — slow start and end.
    EaseInOut,
    /// CSS-style cubic bezier with control points (x1, y1, x2, y2).
    CubicBezier(f64, f64, f64, f64),
    /// Bounce-out — decelerates then bounces at the end like a ball dropping.
    BounceOut,
    /// Elastic-out — overshoots then oscillates like a spring settling.
    /// Uses default period (0.3) and amplitude.
    ElasticOut,
    /// Elastic-out with custom period — lower values = tighter oscillation,
    /// higher values = looser, lazier spring. Default is 0.3.
    Elastic(f64),
}

impl Easing {
    /// Map linear progress `t` (clamped to 0..=1) through this curve.
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => {
                let inv = 1.0 - t;
                1.0 - inv * inv * inv
            }
            Easing::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::CubicBezier(x1, y1, x2, y2) => {
                cubic_bezier_solve(t, x1, y1, x2, y2)
            }
            Easing::BounceOut => bounce_out(t),
            Easing::ElasticOut => elastic_out(t, 0.3),
            Easing::Elastic(period) => elastic_out(t, period),
        }
    }
}

/// Solve cubic bezier: find y for a given x using Newton-Raphson.
/// Control points are (0,0), (x1,y1), (x2,y2), (1,1).
fn cubic_bezier_solve(x: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let mut t = x;
    for _ in 0..8 {
        let bx = bezier(t, x1, x2);
        let dx = bezier_deriv(t, x1, x2);
        if dx.abs() < 1e-10 {
            break;
        }
        t -= (bx - x) / dx;
        t = t.clamp(0.0, 1.0);
    }
    bezier(t, y1, y2)
}

fn bezier(t: f64, p1: f64, p2: f64) -> f64 {
    let inv = 1.0 - t;
    3.0 * inv * inv * t * p1 + 3.0 * inv * t * t * p2 + t * t * t
}

fn bezier_deriv(t: f64, p1: f64, p2: f64) -> f64 {
    let inv = 1.0 - t;
    3.0 * inv * inv * p1 + 6.0 * inv * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

fn bounce_out(t: f64) -> f64 {
    if t < 1.0 / 2.75 {
        7.5625 * t * t
    } else if t < 2.0 / 2.75 {
        let t = t - 1.5 / 2.75;
        7.5625 * t * t + 0.75
    } else if t < 2.5 / 2.75 {
        let t = t - 2.25 / 2.75;
        7.5625 * t * t + 0.9375
    } else {
        let t = t - 2.625 / 2.75;
        7.5625 * t * t + 0.984375
    }
}

fn elastic_out(t: f64, period: f64) -> f64 {
    if t == 0.0 || t == 1.0 {
        return t;
    }
    let p = period.max(0.01);
    (2.0_f64.powf(-6.0 * t) * ((t - p / 4.0) * std::f64::consts::TAU / p).sin()) + 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_passthrough() {
        for i in 0..=10 {
            let t = i as f64 / 10.0;
            assert!((Easing::Linear.apply(t) - t).abs() < 1e-10);
        }
    }

    #[test]
    fn endpoints() {
        for easing in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
            Easing::CubicBezier(0.25, 0.1, 0.25, 1.0),
            Easing::BounceOut,
            Easing::ElasticOut,
            Easing::Elastic(0.15),
            Easing::Elastic(0.5),
        ] {
            assert!((easing.apply(0.0)).abs() < 1e-6, "{:?} at 0", easing);
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-6, "{:?} at 1", easing);
        }
    }

    #[test]
    fn ease_in_slow_start() {
        // At midpoint, ease-in should be below linear
        assert!(Easing::EaseIn.apply(0.5) < 0.5);
    }

    #[test]
    fn ease_out_fast_start() {
        // At midpoint, ease-out should be above linear
        assert!(Easing::EaseOut.apply(0.5) > 0.5);
    }

    #[test]
    fn ease_in_out_symmetric() {
        let a = Easing::EaseInOut.apply(0.25);
        let b = Easing::EaseInOut.apply(0.75);
        // Should be symmetric: f(0.25) + f(0.75) ≈ 1.0
        assert!((a + b - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cubic_bezier_css_ease() {
        // CSS "ease" = cubic-bezier(0.25, 0.1, 0.25, 1.0)
        let ease = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);
        let mid = ease.apply(0.5);
        // Should be above 0.5 (fast in the middle)
        assert!(mid > 0.5);
    }

    #[test]
    fn bounce_out_endpoints() {
        assert!((Easing::BounceOut.apply(0.0)).abs() < 1e-6);
        assert!((Easing::BounceOut.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn bounce_out_overshoots_then_settles() {
        // Should reach values close to 1.0 before t=1.0 (the bounces)
        let late = Easing::BounceOut.apply(0.9);
        assert!(late > 0.9);
    }

    #[test]
    fn elastic_out_endpoints() {
        assert!((Easing::ElasticOut.apply(0.0)).abs() < 1e-6);
        assert!((Easing::ElasticOut.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn elastic_custom_period() {
        // Tight spring (low period) should overshoot more times
        let tight: Vec<f64> = (0..100).map(|i| Easing::Elastic(0.15).apply(i as f64 / 100.0)).collect();
        let loose: Vec<f64> = (0..100).map(|i| Easing::Elastic(0.5).apply(i as f64 / 100.0)).collect();
        // Count zero-crossings around 1.0 — tight should have more
        let crossings = |vals: &[f64]| vals.windows(2).filter(|w| (w[0] - 1.0).signum() != (w[1] - 1.0).signum()).count();
        assert!(crossings(&tight) > crossings(&loose));
    }

    #[test]
    fn elastic_out_overshoots() {
        // Elastic should overshoot past 1.0 at some point
        let values: Vec<f64> = (0..100).map(|i| Easing::ElasticOut.apply(i as f64 / 100.0)).collect();
        assert!(values.iter().any(|&v| v > 1.0), "elastic should overshoot");
    }

    #[test]
    fn clamps_out_of_range() {
        assert!((Easing::EaseIn.apply(-0.5)).abs() < 1e-10);
        assert!((Easing::EaseIn.apply(1.5) - 1.0).abs() < 1e-10);
    }
}
