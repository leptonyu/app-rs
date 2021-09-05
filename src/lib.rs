use std::{
    any::{Any, TypeId},
    collections::{hash_map::Entry, HashMap},
    ops::Deref,
    sync::{Arc, Mutex},
};

use cfg_rs::{ConfigError, Configuration, FromConfig, FromConfigWithPrefix};

pub struct Application {
    config: Configuration,
    cache: Cache,
}

impl From<Configuration> for Application {
    fn from(config: Configuration) -> Self {
        Application::new(config)
    }
}

#[allow(clippy::type_complexity)]
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
    namespace: &'a str,
}

impl AppContext<'_> {
    pub fn get_conf<T: FromConfig>(&self, key: &str) -> Result<T, ConfigError> {
        self.app.config.get::<T>(key)
    }

    pub fn get_namespace(&self) -> &str {
        self.namespace
    }
}

impl Deref for AppContext<'_> {
    type Target = Application;

    fn deref(&self) -> &Self::Target {
        self.app
    }
}

pub trait Resource: Any + Send + Sync + Sized {
    type Config: FromConfigWithPrefix;

    fn create(config: Self::Config, context: &AppContext<'_>) -> Result<Self, ConfigError>;
}

impl Application {
    pub fn new(config: Configuration) -> Self {
        Self {
            config,
            cache: Cache(Mutex::new(HashMap::new())),
        }
    }

    pub fn get_or_new<R: Resource>(&self, namespace: &str) -> Result<R, ConfigError> {
        let c = self
            .config
            .get::<R::Config>(&format!("{}.{}", R::Config::prefix(), namespace))?;
        R::create(
            c,
            &AppContext {
                app: self,
                namespace,
            },
        )
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

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use cfg_rs::*;

    use crate::*;

    #[derive(FromConfig, Debug)]
    #[config(prefix = "hello")]
    struct U {
        #[config(default = "${random.u8}")]
        u: u8,
    }

    impl Resource for U {
        type Config = U;

        fn create(config: Self::Config, _: &AppContext<'_>) -> Result<Self, ConfigError> {
            Ok(config)
        }
    }

    #[test]
    fn u8_test() -> Result<(), ConfigError> {
        let app = Application::new(Configuration::new());
        let u = app.get::<U>("hello")?;
        println!("{}", u.u);
        for _ in 0..10 {
            assert_eq!(&u.u, &app.get::<U>("hello")?.u);
        }
        Ok(())
    }

    #[test]
    fn fun_test() -> Result<(), ConfigError> {
        let app = Application::new(Configuration::new());
        app.get::<U>("")?;
        Ok(())
    }

    #[test]
    #[should_panic]
    fn panic_test() {
        let app = Application::new(Configuration::new());
        app.get::<Arc<U>>("").unwrap();
    }
}
