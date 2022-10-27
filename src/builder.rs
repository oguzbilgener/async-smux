use std::time::Duration;

use crate::{
    config::{MuxConfig, StreamIdType},
    mux::{MuxWorker, TokioConn},
    mux_connection, MuxAcceptor, MuxConnector,
};

pub struct WithConnection<T> {
    config: MuxConfig,
    connection: T,
}

pub struct WithConfig {
    config: MuxConfig,
}

pub struct Begin {}

pub struct MuxBuilder<State> {
    state: State,
}

impl MuxBuilder<Begin> {
    pub fn client() -> MuxBuilder<WithConfig> {
        MuxBuilder {
            state: WithConfig {
                config: MuxConfig {
                    stream_id_type: StreamIdType::Odd,
                    keep_alive_interval: None,
                },
            },
        }
    }

    pub fn server() -> MuxBuilder<WithConfig> {
        MuxBuilder {
            state: WithConfig {
                config: MuxConfig {
                    stream_id_type: StreamIdType::Even,
                    keep_alive_interval: None,
                },
            },
        }
    }
}

impl MuxBuilder<WithConfig> {
    pub fn with_keep_alive_interval(&mut self, interval: Duration) -> &mut Self {
        self.state.config.keep_alive_interval = Some(interval);
        self
    }

    pub fn with_connection<T: TokioConn>(
        &mut self,
        connection: T,
    ) -> MuxBuilder<WithConnection<T>> {
        MuxBuilder {
            state: WithConnection {
                config: self.state.config,
                connection,
            },
        }
    }
}

impl<T: TokioConn> MuxBuilder<WithConnection<T>> {
    pub fn build(self) -> (MuxConnector<T>, MuxAcceptor<T>, MuxWorker<T>) {
        mux_connection(self.state.connection, self.state.config)
    }
}
