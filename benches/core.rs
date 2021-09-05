use app_rs::{Application, Resource};
use cfg_rs::*;
use criterion::{criterion_group, criterion_main, Criterion};

#[derive(FromConfig, Debug)]
#[config(prefix = "hello")]
struct X {
    #[config(default = "${random.u64}")]
    _r: u64,
}

impl Resource for X {
    type Config = X;

    fn create(
        config: Self::Config,
        _: &app_rs::AppContext<'_>,
    ) -> Result<Self, cfg_rs::ConfigError> {
        Ok(config)
    }

    fn prefix_key() -> String {
        "".to_string()
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let env = Configuration::new().register_random().unwrap();
    let app = Application::new(env);

    let x = app.get::<X>("").unwrap();

    println!("{}", x._r);

    c.bench_function("res", |b| {
        b.iter(|| assert_eq!(x._r, app.get::<X>("").unwrap()._r))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
