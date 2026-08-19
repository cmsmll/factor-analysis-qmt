/// 滑动窗口统计：标准差、偏度、峰度、线性回归斜率。
///
/// 统一维护窗口缓冲与累计和/平方和，支持 O(1) 更新的均值与方差。

#[derive(Clone)]
pub struct WindowStats {
    idx: usize,
    len: usize,
    sum: f64,
    sum_sq: f64,
    buf: Vec<f64>,
}

impl WindowStats {
    /// 创建一个周期为 `len` 的滑动窗口统计器（`len >= 2`）。
    pub fn new(len: usize) -> Self {
        assert!(len >= 2, "WindowStats 周期 len 必须大于等于 2");

        Self {
            idx: 0,
            len,
            sum: 0.0,
            sum_sq: 0.0,
            buf: vec![0.0; len],
        }
    }

    /// 输入一个新值并返回当前窗口均值；预热未完成时返回 `None`。
    pub fn mean(&mut self, value: f64) -> Option<f64> {
        let pos = self.idx % self.len;
        let old = self.buf[pos];
        self.buf[pos] = value;
        self.sum += value - old;
        self.sum_sq += value * value - old * old;
        self.idx += 1;

        (self.idx >= self.len).then_some(self.sum / self.len as f64)
    }

    /// 输入一个新值并返回当前窗口总体标准差；预热未完成时返回 `None`。
    ///
    /// 使用 `sqrt(mean(x²) - mean(x)²)`，窗口内方差非负，浮点误差钳制到 0。
    pub fn std(&mut self, value: f64) -> Option<f64> {
        let len = self.len as f64;
        self.mean(value).map(|mean| {
            let variance = (self.sum_sq / len - mean * mean).max(0.0);
            variance.sqrt()
        })
    }

    /// 输入一个新值并返回当前窗口偏度（三阶矩标准化）；预热未完成时返回 `None`。
    ///
    /// 偏度 = (1/N * Σ(x-μ)³) / σ³，σ=0 时返回 0。
    pub fn skewness(&mut self, value: f64) -> Option<f64> {
        let mean = self.mean(value)?;
        let std = self.std_of_buf(mean);
        if std == 0.0 {
            return Some(0.0);
        }
        let n = self.len as f64;
        let third = self.buf.iter().map(|v| (v - mean).powi(3)).sum::<f64>() / n;
        Some(third / std.powi(3))
    }

    /// 输入一个新值并返回当前窗口峰度（四阶矩标准化，超额峰度）；预热未完成时返回 `None`。
    ///
    /// 峰度 = (1/N * Σ(x-μ)⁴) / σ⁴ - 3，σ=0 时返回 0。
    pub fn kurtosis(&mut self, value: f64) -> Option<f64> {
        let mean = self.mean(value)?;
        let std = self.std_of_buf(mean);
        if std == 0.0 {
            return Some(0.0);
        }
        let n = self.len as f64;
        let fourth = self.buf.iter().map(|v| (v - mean).powi(4)).sum::<f64>() / n;
        Some(fourth / std.powi(4) - 3.0)
    }

    fn std_of_buf(&self, mean: f64) -> f64 {
        let n = self.len as f64;
        let variance = self.buf.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
        variance.sqrt()
    }
}

/// 滑动窗口线性回归斜率（y = 价格，x = 时间序号 0..len-1）。
///
/// 斜率 = Σ((x-x̄)(y-ȳ)) / Σ((x-x̄)²)。窗口内价格全相等时返回 0。
#[derive(Clone)]
pub struct LinReg {
    idx: usize,
    len: usize,
    sum_y: f64,
    buf: Vec<f64>,
    x_centered: Vec<f64>,
    denom: f64,
}

impl LinReg {
    /// 创建一个周期为 `len` 的线性回归斜率器（`len >= 2`）。
    pub fn new(len: usize) -> Self {
        assert!(len >= 2, "LinReg 周期 len 必须大于等于 2");

        let x_mean = (len as f64 - 1.0) / 2.0;
        let x_centered = (0..len).map(|i| i as f64 - x_mean).collect::<Vec<_>>();
        let denom = x_centered.iter().map(|x| x * x).sum::<f64>();

        Self {
            idx: 0,
            len,
            sum_y: 0.0,
            buf: vec![0.0; len],
            x_centered,
            denom,
        }
    }

    /// 输入一个新价格并返回当前窗口回归斜率；预热未完成时返回 `None`。
    pub fn next(&mut self, price: f64) -> Option<f64> {
        let pos = self.idx % self.len;
        let old = self.buf[pos];
        self.buf[pos] = price;
        self.sum_y += price - old;
        self.idx += 1;

        if self.idx < self.len {
            return None;
        }

        let y_mean = self.sum_y / self.len as f64;
        // 环形缓冲：最新值在 pos=(idx-1)%len，需按时间顺序（旧→新）遍历。
        let start = self.idx % self.len;
        let mut numerator = 0.0;
        for (i, x) in self.x_centered.iter().enumerate() {
            let physical = (start + i) % self.len;
            let y = self.buf[physical];
            numerator += x * (y - y_mean);
        }

        Some(crate::math::dev(numerator, self.denom))
    }
}

#[cfg(test)]
mod tests {
    use super::{LinReg, WindowStats};

    #[test]
    fn std_returns_none_before_warmup() {
        let mut stats = WindowStats::new(3);
        assert_eq!(stats.std(1.0), None);
        assert_eq!(stats.std(2.0), None);
        assert!(stats.std(3.0).is_some());
    }

    #[test]
    fn std_computes_population_std() {
        let mut stats = WindowStats::new(4);
        for value in [1.0, 2.0, 3.0, 4.0] {
            stats.std(value);
        }
        let std = stats.std(5.0).unwrap();
        // 窗口 [2,3,4,5] 的总体标准差 = sqrt(1.25)
        assert!((std - 1.25f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn skewness_zero_for_constant_series() {
        let mut stats = WindowStats::new(3);
        for _ in 0..3 {
            stats.skewness(1.0);
        }
        assert_eq!(stats.skewness(1.0), Some(0.0));
    }

    #[test]
    fn kurtosis_zero_for_constant_series() {
        let mut stats = WindowStats::new(3);
        for _ in 0..3 {
            stats.kurtosis(1.0);
        }
        assert_eq!(stats.kurtosis(1.0), Some(0.0));
    }

    #[test]
    fn linreg_returns_none_before_warmup() {
        let mut reg = LinReg::new(3);
        assert_eq!(reg.next(1.0), None);
        assert_eq!(reg.next(2.0), None);
        assert!(reg.next(3.0).is_some());
    }

    #[test]
    fn linreg_slope_of_linear_series() {
        // y = 2x + 1：斜率应为 2
        let mut reg = LinReg::new(4);
        for value in [1.0, 3.0, 5.0, 7.0] {
            reg.next(value);
        }
        let slope = reg.next(9.0).unwrap();
        assert!((slope - 2.0).abs() < 1e-12);
    }

    #[test]
    fn linreg_slope_zero_for_constant() {
        let mut reg = LinReg::new(3);
        for _ in 0..3 {
            reg.next(5.0);
        }
        assert_eq!(reg.next(5.0), Some(0.0));
    }
}
