use app_rs::{Application, Resource};
use cfg_rs::Configuration;
use criterion::{criterion_group, criterion_main, Criterion};

struct X {
    _r: u64,
}

impl Resource for X {
    type Config = u64;

    fn create(
        config: Self::Config,
        _: &app_rs::AppContext<'_>,
    ) -> Result<Self, cfg_rs::ConfigError> {
        Ok(X { _r: config })
    }
}

fn criterion_benchmark(c: &mut Criterion) {
    let env = Configuration::with_predefined_builder()
        .set("hello", "${random.u64}")
        .init()
        .unwrap();
    let app = Application::new(env);

    let x = app.get::<X>("hello").unwrap();

    println!("{}", x._r);

    c.bench_function("res", |b| {
        b.iter(|| assert_eq!(x._r, app.get::<X>("hello").unwrap()._r))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
