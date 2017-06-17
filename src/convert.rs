pub trait IntoParameter {
    fn into_parameter(self) -> String;
}

impl IntoParameter for &'static str {
    fn into_parameter(self) -> String {
        self.to_string()
    }
}

impl IntoParameter for f64 {
    fn into_parameter(self) -> String {
        self.to_string()
    }
}

impl IntoParameter for u64 {
    fn into_parameter(self) -> String {
        self.to_string()
    }
}

impl IntoParameter for bool {
    fn into_parameter(self) -> String {
        match self {
            true  => "1",
            false => "0",
        }.to_string()
    }
}

impl<A, B> IntoParameter for (A, B)
where A: IntoParameter,
      B: IntoParameter {
    fn into_parameter(self) -> String {
        format!("{} {}", self.0.into_parameter(), self.1.into_parameter())
    }
}

impl<A, B, C> IntoParameter for (A, B, C)
where A: IntoParameter,
      B: IntoParameter,
      C: IntoParameter {
    fn into_parameter(self) -> String {
        format!("{} {} {}", self.0.into_parameter(), self.1.into_parameter(), self.2.into_parameter())
    }
}
