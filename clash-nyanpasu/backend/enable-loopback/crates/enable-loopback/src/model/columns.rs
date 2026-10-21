//! Shared DIP column geometry, independent of reactor and native windows.
use super::Column;

pub const CHECKBOX_WIDTH: f64 = 40.0;
pub const MIN_COLUMN_WIDTH: f64 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnWidths {
    widths: [f64; 7],
    minimums: [f64; 7],
}

impl Default for ColumnWidths {
    fn default() -> Self {
        Self::new(
            [220.0, 220.0, 200.0, 160.0, 140.0, 140.0, MIN_COLUMN_WIDTH],
            [MIN_COLUMN_WIDTH; 7],
        )
    }
}

impl ColumnWidths {
    pub fn new(initial: [f64; 7], minimums: [f64; 7]) -> Self {
        let minimums = minimums.map(|width| {
            if width.is_finite() {
                width.max(MIN_COLUMN_WIDTH)
            } else {
                MIN_COLUMN_WIDTH
            }
        });
        let widths = std::array::from_fn(|index| {
            if initial[index].is_finite() {
                initial[index].max(minimums[index])
            } else {
                minimums[index]
            }
        });
        Self { widths, minimums }
    }

    pub fn get(self, column: Column) -> f64 {
        self.widths[column as usize]
    }

    /// Use the press origin, rather than accumulating rounded move deltas.
    pub fn drag(&mut self, column: Column, initial_width: f64, delta: f64) {
        let width = initial_width + delta;
        if width.is_finite() {
            self.widths[column as usize] = width.max(self.minimums[column as usize]);
        }
    }

    pub fn total(self) -> f64 {
        CHECKBOX_WIDTH + self.widths.iter().sum::<f64>()
    }

    /// Only Binaries grows to fill spare viewport width; stored sizes stay intact.
    pub fn resolved(self, viewport: f64) -> [f64; 8] {
        let mut widths = [CHECKBOX_WIDTH; 8];
        widths[1..].copy_from_slice(&self.widths);
        widths[7] += (viewport - self.total()).max(0.0);
        widths
    }

    pub fn column_at(self, x: f64) -> Column {
        let mut edge = CHECKBOX_WIDTH;
        for column in [
            Column::DisplayName,
            Column::Description,
            Column::Package,
            Column::Name,
            Column::Sid,
            Column::Users,
        ] {
            edge += self.get(column);
            if x < edge {
                return column;
            }
        }
        Column::Binaries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurable_widths_enforce_finite_initial_sizes_and_per_column_minimums() {
        let mut widths = ColumnWidths::new([f64::NAN; 7], [72.0; 7]);
        assert_eq!(widths.get(Column::DisplayName), 72.0);
        widths.drag(Column::DisplayName, 72.0, -100.0);
        assert_eq!(widths.get(Column::DisplayName), 72.0);
        let widths = ColumnWidths::new([0.0; 7], [f64::INFINITY; 7]);
        assert_eq!(
            widths.resolved(0.0),
            [40.0, 48.0, 48.0, 48.0, 48.0, 48.0, 48.0, 48.0]
        );
    }

    #[test]
    fn drag_clamps_without_accumulating_and_rejects_nonfinite_input() {
        let mut widths = ColumnWidths::default();
        widths.drag(Column::Name, 160.0, -500.0);
        assert_eq!(widths.get(Column::Name), 48.0);
        widths.drag(Column::Name, 160.0, 25.5);
        assert_eq!(widths.get(Column::Name), 185.5);
        widths.drag(Column::Name, 160.0, f64::NAN);
        assert_eq!(widths.get(Column::Name), 185.5);
        widths.drag(Column::Name, 160.0, f64::INFINITY);
        assert_eq!(widths.get(Column::Name), 185.5);
    }

    #[test]
    fn total_and_fill_preserve_fixed_columns_and_binaries_minimum() {
        let widths = ColumnWidths::default();
        assert_eq!(widths.total(), 1168.0);
        assert_eq!(
            widths.resolved(900.0),
            [40.0, 220.0, 220.0, 200.0, 160.0, 140.0, 140.0, 48.0]
        );
        assert_eq!(widths.resolved(1400.0)[7], 280.0);
        let mut resized = widths;
        resized.drag(Column::Binaries, 280.0, 30.0);
        assert_eq!(resized.total(), 1430.0);
        assert_eq!(resized.resolved(1400.0)[7], 310.0);
        assert_eq!(resized.resolved(1600.0)[7], 480.0);
    }

    #[test]
    fn hit_testing_uses_resized_boundaries_in_table_local_coordinates() {
        let mut widths = ColumnWidths::default();
        assert_eq!(widths.column_at(259.9), Column::DisplayName);
        assert_eq!(widths.column_at(260.0), Column::Description);
        widths.drag(Column::DisplayName, 220.0, 100.0);
        assert_eq!(widths.column_at(359.9), Column::DisplayName);
        assert_eq!(widths.column_at(360.0), Column::Description);
        assert_eq!(widths.column_at(1220.0), Column::Binaries);
    }
}
