#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    In,
    Out,
    InOut,
}

impl Easing {
    pub fn from_name(name: &str) -> Option<Easing> {
        match name {
            "linear" => Some(Easing::Linear),
            "in" => Some(Easing::In),
            "out" => Some(Easing::Out),
            "inout" => Some(Easing::InOut),
            _ => None,
        }
    }

    /// Maps progress `u` in 0..=1 to eased progress, using cubic curves.
    pub fn apply(self, u: f64) -> f64 {
        match self {
            Easing::Linear => u,
            Easing::In => u * u * u,
            Easing::Out => 1.0 - (1.0 - u).powi(3),
            Easing::InOut => {
                if u < 0.5 {
                    4.0 * u * u * u
                } else {
                    1.0 - (-2.0 * u + 2.0).powi(3) / 2.0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_easings_start_at_zero_and_end_at_one() {
        for easing in [Easing::Linear, Easing::In, Easing::Out, Easing::InOut] {
            assert_eq!(easing.apply(0.0), 0.0);
            assert_eq!(easing.apply(1.0), 1.0);
        }
    }

    #[test]
    fn inout_is_half_way_at_the_middle() {
        assert_eq!(Easing::InOut.apply(0.5), 0.5);
    }
}
