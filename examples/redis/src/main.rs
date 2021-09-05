use std::time::Duration;

use app_rs::{AppContext, Resource};
use cfg_rs::*;
use r2d2::{ManageConnection, Pool};
use redis::{cluster::*, cmd, RedisError};
use scheduled_thread_pool::ScheduledThreadPool;

/// Generic Pool Configuration.
#[derive(FromConfig, Debug)]
#[config(prefix = "pool")]
pub struct PoolConfig {
    max_size: Option<u32>,
    min_idle: Option<u32>,
    thread_name: Option<String>,
    thread_nums: Option<usize>,
    test_on_check_out: Option<bool>,
    max_lifetime: Option<Duration>,
    idle_timeout: Option<Duration>,
    #[config(default = "1s")]
    connection_timeout: Option<Duration>,
    #[config(default = "false")]
    wait_for_init: bool,
}

macro_rules! set_option_field_return {
    ($y: ident, $config: ident, $x: tt) => {
        if let Some($x) = $y.$x {
            $config = $config.$x($x);
        }
    };
}

struct PMC<M: ManageConnection>(Pool<M>);

impl<M: ManageConnection + Resource> Resource for PMC<M> {
    type Config = PoolConfig;

    fn create(config: Self::Config, context: &AppContext<'_>) -> Result<Self, ConfigError> {
        let thread_nums = config.thread_nums.unwrap_or(3);
        let mut build: r2d2::Builder<M> = Pool::builder()
            .min_idle(config.min_idle)
            .max_lifetime(config.max_lifetime)
            .idle_timeout(config.idle_timeout)
            .thread_pool(std::sync::Arc::new(match config.thread_name {
                Some(name) => ScheduledThreadPool::with_name(&name, thread_nums),
                None => ScheduledThreadPool::new(thread_nums),
            }));
        set_option_field_return!(config, build, connection_timeout);
        set_option_field_return!(config, build, max_size);
        set_option_field_return!(config, build, test_on_check_out);

        let m = context.get_or_new::<M>(context.get_namespace())?;

        Ok(PMC(if config.wait_for_init {
            build.build(m)?
        } else {
            build.build_unchecked(m)
        }))
    }
}

#[derive(FromConfig, Debug)]
#[config(prefix = "redis")]
pub struct RedisClusterConfig {
    url: Vec<String>,
    password: Option<String>,
    readonly: Option<bool>,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    auto_reconnect: Option<bool>,
    pool: PoolConfig,
}

#[allow(missing_debug_implementations)]
pub struct RedisClusterConnectionManager {
    #[allow(dead_code)]
    namespace: &'static str,
    client: ClusterClient,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    auto_reconnect: Option<bool>,
}

impl ManageConnection for RedisClusterConnectionManager {
    type Connection = ClusterConnection;
    type Error = RedisError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = self.client.get_connection()?;
        if let Some(auto_reconnect) = self.auto_reconnect {
            conn.set_auto_reconnect(auto_reconnect);
        }
        conn.set_read_timeout(self.read_timeout)?;
        conn.set_write_timeout(self.write_timeout)?;
        Ok(conn)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        cmd("PING").query(conn)
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        !conn.check_connection()
    }
}

#[allow(missing_debug_implementations)]
pub struct RedisClusterPool(Pool<RedisClusterConnectionManager>);

fn main() {
    println!("Hello, world!");
}
