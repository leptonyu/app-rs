use std::{
    any::{Any, TypeId},
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, Mutex},
};

use cfg_rs::{ConfigError, Configuration, FromConfig};

pub struct Application {
    config: Configuration,
    cache: Cache,
}

impl From<Configuration> for Application {
    fn from(config: Configuration) -> Self {
        Application::new(config)
    }
}

struct Cache(Mutex<HashMap<TypeId, HashMap<String, Arc<dyn Any + Send + Sync + 'static>>>>);

impl Cache {
    fn get<T: Any + Send + Sync, F: FnOnce() -> Result<T, ConfigError>>(
        &self,
        namespace: &str,
        f: F,
    ) -> Result<Arc<T>, ConfigError> {
        let mut g = match self.0.try_lock() {
            Ok(v) => v,
            Err(_) => return Err(ConfigError::RefValueRecursiveError),
        };
        Ok(
            match g
                .entry(TypeId::of::<T>())
                .or_insert_with(HashMap::new)
                .entry(namespace.to_string())
            {
                Entry::Occupied(mut v) => v.get_mut().clone().downcast::<T>().unwrap(),
                Entry::Vacant(v) => v.insert(Arc::new((f)()?)).clone().downcast::<T>().unwrap(),
            },
        )
    }
}

pub struct AppContext<'a> {
    app: &'a Application,
}

impl AppContext<'_> {
    pub fn get_conf<T: FromConfig>(&self, key: &str) -> Result<T, ConfigError> {
        self.app.config.get::<T>(key)
    }
}

pub trait Resource: Any + Send + Sync + Sized {
    type Config: FromConfig;

    fn create(config: Self::Config, context: &AppContext<'_>) -> Result<Self, ConfigError>;
}

impl Application {
    pub fn new(config: Configuration) -> Self {
        Self {
            config,
            cache: Cache(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_new<R: Resource>(&self, key: &str) -> Result<R, ConfigError> {
        let c = self.config.get::<R::Config>(&key)?;
        R::create(c, &AppContext { app: &self })
    }

    pub fn get<R: Resource>(&self, key: &str) -> Result<Arc<R>, ConfigError> {
        self.get_or_new::<Arc<R>>(key)
    }
}

impl<T: Resource> Resource for Arc<T> {
    type Config = T::Config;

    fn create(config: Self::Config, context: &AppContext<'_>) -> Result<Self, ConfigError> {
        context.app.cache.get("", || T::create(config, context))
    }
}

macro_rules! impl_primitive {
    ($x:ty) => {
        impl Resource for $x {
            type Config = $x;

            fn create(x: Self::Config, _: &AppContext<'_>) -> Result<Self, ConfigError> {
                Ok(x)
            }
        }
    };
}

impl_primitive!(());

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use cfg_rs::{ConfigError, Configuration};

    use crate::*;

    impl_primitive!(u8);

    #[test]
    fn u8_test() -> Result<(), ConfigError> {
        let app = Application::new(
            Configuration::new()
                .register_random()
                .unwrap()
                .register_kv("name")
                .set("hello", "${random.u8}")
                .finish()
                .unwrap(),
        );
        let u = app.get::<u8>("hello")?;
        println!("{}", u);
        for _ in 0..10 {
            assert_eq!(&u, &app.get::<u8>("hello")?);
        }
        Ok(())
    }

    #[test]
    fn fun_test() -> Result<(), ConfigError> {
        let app = Application::new(Configuration::new());
        app.get::<()>("")?;
        Ok(())
    }

    #[test]
    #[should_panic]
    fn panic_test() {
        let app = Application::new(Configuration::new());
        app.get::<Arc<()>>("").unwrap();
    }
}
