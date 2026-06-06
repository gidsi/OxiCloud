#!/bin/bash

# Extract parts before and after conflict
head -n 427 src/main.rs > src/main.rs.new

cat << 'INNER' >> src/main.rs.new
        // Magic-link redemption — public, no CSRF, no rate limit (the token IS
        // the credential and `mark_used` is single-use). PR 12 will add a
        // per-IP limiter on top.
        let magic_link_router = interfaces::api::handlers::magic_link_handler::magic_link_routes()
            .with_state(app_state.clone());

        let carddav_protected = carddav_router
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                require_internal_user_layer,
            ))
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                auth_middleware,
            ))
            .layer(axum::middleware::from_fn_with_state(
                dav_options_limiter.clone(),
                rate_limit_dav_options,
            ));

        let webdav_protected = webdav_router
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                require_internal_user_layer,
            ))
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                auth_middleware,
            ));
INNER

tail -n +472 src/main.rs >> src/main.rs.new

mv src/main.rs.new src/main.rs
