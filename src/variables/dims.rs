//! `Dims`: the shape of an array variable, as `<Dimension>` writes it.

/// The fixed `<Dimension>`s of a variable, outermost first: `[[f32; 3]; 2]` is `[2, 3]`.
/// A scalar has none. Built in `const`. More than eight dimensions fail to compile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dims {
    len: u8,
    sizes: [usize; 8],
}

impl Dims {
    pub const SCALAR: Self = Self {
        len: 0,
        sizes: [0; 8],
    };

    /// These dimensions with `n` added outside them.
    #[must_use]
    pub const fn outer(self, n: usize) -> Self {
        let mut sizes = [0; 8];
        sizes[0] = n;
        let mut i = 0;
        while i < self.len as usize {
            sizes[i + 1] = self.sizes[i];
            i += 1;
        }
        Self {
            len: self.len + 1,
            sizes,
        }
    }

    #[must_use]
    pub fn as_slice(&self) -> &[usize] {
        &self.sizes[..usize::from(self.len)]
    }

    /// The number of scalar values: 1 for a scalar.
    #[must_use]
    pub const fn count(&self) -> usize {
        let mut count = 1;
        let mut i = 0;
        while i < self.len as usize {
            count *= self.sizes[i];
            i += 1;
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outer_puts_the_new_dimension_first() {
        let dims = Dims::SCALAR.outer(3).outer(2);
        assert_eq!(dims.as_slice(), &[2, 3]);
        assert_eq!(dims.count(), 6);
        assert_eq!(Dims::SCALAR.count(), 1);
    }
}
