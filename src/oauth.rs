use color_eyre::{
    Section as _,
    eyre::{self, Context as _},
};
use librespot_core::SessionConfig;
use librespot_core::{Session, authentication::Credentials};
use log::info;
use tokio::runtime::Runtime;

use crate::{LogTarget, config::CliConfig, setup_logger};

pub(crate) fn run_oauth(mut cli_config: CliConfig, oauth_port: u16) -> eyre::Result<()> {
    setup_logger(LogTarget::Terminal, cli_config.verbose)?;

    cli_config
        .load_config_file_values()
        .wrap_err("failed to read config file")?;

    let cache = cli_config
        .shared_config
        .get_cache(true)
        .with_note(|| "The result of the authentication needs to be cached to be usable later.")?;

    const OAUTH_SCOPES: &[&str] = &[
        "app-remote-control",
        "playlist-modify",
        "playlist-modify-private",
        "playlist-modify-public",
        "playlist-read",
        "playlist-read-collaborative",
        "playlist-read-private",
        "streaming",
        "ugc-image-upload",
        "user-follow-modify",
        "user-follow-read",
        "user-library-modify",
        "user-library-read",
        "user-modify",
        "user-modify-playback-state",
        "user-modify-private",
        "user-personalized",
        "user-read-birthdate",
        "user-read-currently-playing",
        "user-read-email",
        "user-read-play-history",
        "user-read-playback-position",
        "user-read-playback-state",
        "user-read-private",
        "user-read-recently-played",
        "user-top-read",
    ];

    let session_config = SessionConfig {
        proxy: cli_config.shared_config.proxy_url(),
        ..Default::default()
    };

    let client = librespot_oauth::OAuthClientBuilder::new(
        &session_config.client_id,
        &format!("http://127.0.0.1:{oauth_port}/login"),
        OAUTH_SCOPES.to_vec(),
    )
    .with_custom_message(r#"<h3 style="color: darkgreen; align: center;">Authentication successful! You can now return to spotifyd.</h3>"#)
    .open_in_browser()
    .build()
    .wrap_err("token retrieval failed")?;

    Runtime::new().unwrap().block_on(async move {
        let token = client.get_access_token_async().await?;
        let creds = Credentials::with_access_token(token.access_token);

        let session = Session::new(session_config, Some(cache));
        session.connect(creds, true).await
    })?;

    info!("\nLogin successful! You are now ready to run spotifyd.");

    Ok(())
}
