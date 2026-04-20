use anneal_core::{Actor, ApplicationError, ApplicationResult};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::{RngExt, distr::Alphanumeric};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use totp_rs::{Algorithm as TotpAlgorithm, Secret, TOTP};
use uuid::Uuid;

use crate::{
    application::{AccessTokenService, PasswordService, SessionRepository, TotpService},
    domain::{AccessClaims, PreAuthChallenge, PreAuthPurpose, RefreshSession, TotpSetup},
};

#[derive(Clone)]
pub struct JwtService {
    access_secret: String,
    pre_auth_secret: String,
}

impl JwtService {
    pub fn new(access_secret: impl Into<String>, pre_auth_secret: impl Into<String>) -> Self {
        Self {
            access_secret: access_secret.into(),
            pre_auth_secret: pre_auth_secret.into(),
        }
    }

    fn issue(
        &self,
        actor: &Actor,
        kind: &str,
        challenge_id: Option<Uuid>,
        purpose: Option<PreAuthPurpose>,
        ttl: Duration,
        secret: &str,
    ) -> ApplicationResult<(String, chrono::DateTime<Utc>)> {
        let issued_at = Utc::now();
        let expires_at = issued_at + ttl;
        let claims = AccessClaims {
            sub: actor.user_id,
            role: actor.role,
            tenant_id: actor.tenant_id,
            kind: kind.into(),
            challenge_id,
            purpose: purpose.map(|value| value.as_str().into()),
            exp: expires_at.timestamp() as usize,
            iat: issued_at.timestamp() as usize,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok((token, expires_at))
    }
}

impl AccessTokenService for JwtService {
    fn issue_access_token(
        &self,
        actor: &Actor,
    ) -> ApplicationResult<(String, chrono::DateTime<Utc>)> {
        self.issue(
            actor,
            "access",
            None,
            None,
            Duration::minutes(15),
            &self.access_secret,
        )
    }

    fn issue_pre_auth_token(
        &self,
        actor: &Actor,
        challenge_id: Uuid,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<String> {
        self.issue(
            actor,
            "pre_auth",
            Some(challenge_id),
            Some(purpose),
            Duration::minutes(10),
            &self.pre_auth_secret,
        )
        .map(|(token, _)| token)
    }

    fn decode_claims(&self, token: &str) -> ApplicationResult<AccessClaims> {
        let access = decode::<AccessClaims>(
            token,
            &DecodingKey::from_secret(self.access_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        );
        if let Ok(data) = access {
            return Ok(data.claims);
        }
        decode::<AccessClaims>(
            token,
            &DecodingKey::from_secret(self.pre_auth_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .map(|data| data.claims)
        .map_err(|_| ApplicationError::Unauthorized)
    }
}

#[derive(Clone, Copy)]
pub struct ArgonPasswordService;

#[async_trait]
impl PasswordService for ArgonPasswordService {
    async fn hash_password(&self, password: &str) -> ApplicationResult<String> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|value| value.to_string())
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn verify_password(
        &self,
        password: &str,
        password_hash: &str,
    ) -> ApplicationResult<bool> {
        let parsed_hash = PasswordHash::new(password_hash)
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }
}

#[derive(Clone)]
pub struct OtpAuthTotpService {
    issuer: String,
}

impl OtpAuthTotpService {
    pub fn new(issuer: impl Into<String>) -> Self {
        Self {
            issuer: issuer.into(),
        }
    }
}

impl TotpService for OtpAuthTotpService {
    fn generate(&self, email: &str) -> ApplicationResult<TotpSetup> {
        let secret = Secret::generate_secret();
        let Secret::Encoded(encoded) = secret.to_encoded() else {
            return Err(ApplicationError::Infrastructure(
                "failed to encode totp secret".into(),
            ));
        };
        self.build(&encoded, email)
    }

    fn build(&self, secret: &str, email: &str) -> ApplicationResult<TotpSetup> {
        let totp = TOTP::new(
            TotpAlgorithm::SHA1,
            6,
            1,
            30,
            Secret::Encoded(secret.into())
                .to_bytes()
                .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?,
            Some(self.issuer.clone()),
            email.to_string(),
        )
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(TotpSetup {
            secret: secret.into(),
            otpauth_url: totp.get_url(),
        })
    }

    fn verify(&self, secret: &str, code: &str, email: &str) -> ApplicationResult<bool> {
        let bytes = Secret::Encoded(secret.into())
            .to_bytes()
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let totp = TOTP::new(
            TotpAlgorithm::SHA1,
            6,
            1,
            30,
            bytes,
            Some(self.issuer.clone()),
            email.to_string(),
        )
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        totp.check_current(code)
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }
}

#[derive(Clone)]
pub struct PgSessionRepository {
    pool: PgPool,
}

impl PgSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PgSessionRepository {
    async fn create_session(&self, session: RefreshSession) -> ApplicationResult<RefreshSession> {
        sqlx::query(
            r#"
            insert into refresh_sessions (
                id, user_id, refresh_token_hash, user_agent, ip_address, expires_at, revoked_at, rotated_from_session_id, created_at
            ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(&session.refresh_token_hash)
        .bind(&session.user_agent)
        .bind(&session.ip_address)
        .bind(session.expires_at)
        .bind(session.revoked_at)
        .bind(session.rotated_from_session_id)
        .bind(session.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(session)
    }

    async fn consume_active_session_by_hash(
        &self,
        refresh_token_hash: &str,
    ) -> ApplicationResult<Option<RefreshSession>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let session = sqlx::query_as::<_, RefreshSession>(
            r#"
            select * from refresh_sessions
            where refresh_token_hash = $1
              and revoked_at is null
              and expires_at > now() at time zone 'utc'
            order by created_at desc
            limit 1
            for update
            "#,
        )
        .bind(refresh_token_hash)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        if let Some(session) = &session {
            sqlx::query(
                "update refresh_sessions set revoked_at = now() at time zone 'utc' where id = $1",
            )
            .bind(session.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(session.map(|mut session| {
            session.revoked_at = Some(Utc::now());
            session
        }))
    }

    async fn find_active_session_by_hash(
        &self,
        refresh_token_hash: &str,
    ) -> ApplicationResult<Option<RefreshSession>> {
        sqlx::query_as::<_, RefreshSession>(
            r#"
            select * from refresh_sessions
            where refresh_token_hash = $1 and revoked_at is null
            order by created_at desc
            limit 1
            "#,
        )
        .bind(refresh_token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn revoke_session(&self, session_id: Uuid) -> ApplicationResult<()> {
        sqlx::query(
            "update refresh_sessions set revoked_at = now() at time zone 'utc' where id = $1",
        )
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn list_user_sessions(&self, user_id: Uuid) -> ApplicationResult<Vec<RefreshSession>> {
        sqlx::query_as::<_, RefreshSession>(
            "select * from refresh_sessions where user_id = $1 order by created_at desc",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn revoke_user_sessions(&self, user_id: Uuid) -> ApplicationResult<()> {
        sqlx::query(
            "update refresh_sessions set revoked_at = now() at time zone 'utc' where user_id = $1 and revoked_at is null",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn create_pre_auth_challenge(
        &self,
        challenge: PreAuthChallenge,
    ) -> ApplicationResult<PreAuthChallenge> {
        sqlx::query(
            r#"
            insert into pre_auth_challenges (
                id, user_id, purpose, pending_totp_secret, expires_at, used_at, created_at
            ) values ($1,$2,$3,$4,$5,$6,$7)
            "#,
        )
        .bind(challenge.id)
        .bind(challenge.user_id)
        .bind(&challenge.purpose)
        .bind(&challenge.pending_totp_secret)
        .bind(challenge.expires_at)
        .bind(challenge.used_at)
        .bind(challenge.created_at)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(challenge)
    }

    async fn find_active_pre_auth_challenge(
        &self,
        challenge_id: Uuid,
    ) -> ApplicationResult<Option<PreAuthChallenge>> {
        sqlx::query_as::<_, PreAuthChallenge>(
            r#"
            select * from pre_auth_challenges
            where id = $1 and used_at is null and expires_at > now() at time zone 'utc'
            "#,
        )
        .bind(challenge_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))
    }

    async fn update_pre_auth_challenge_secret(
        &self,
        challenge_id: Uuid,
        pending_totp_secret: &str,
    ) -> ApplicationResult<()> {
        sqlx::query(
            "update pre_auth_challenges set pending_totp_secret = $2 where id = $1 and used_at is null and expires_at > now() at time zone 'utc'",
        )
        .bind(challenge_id)
        .bind(pending_totp_secret)
        .execute(&self.pool)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn consume_pre_auth_challenge(
        &self,
        challenge_id: Uuid,
        user_id: Uuid,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<Option<PreAuthChallenge>> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        let challenge = sqlx::query_as::<_, PreAuthChallenge>(
            r#"
            select * from pre_auth_challenges
            where id = $1
              and user_id = $2
              and purpose = $3
              and used_at is null
              and expires_at > now() at time zone 'utc'
            for update
            "#,
        )
        .bind(challenge_id)
        .bind(user_id)
        .bind(purpose.as_str())
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        if let Some(challenge) = &challenge {
            sqlx::query(
                "update pre_auth_challenges set used_at = now() at time zone 'utc' where id = $1",
            )
            .bind(challenge.id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        }
        transaction
            .commit()
            .await
            .map_err(|error| ApplicationError::Infrastructure(error.to_string()))?;
        Ok(challenge.map(|mut challenge| {
            challenge.used_at = Some(Utc::now());
            challenge
        }))
    }
}

pub fn generate_refresh_token() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

pub fn hash_refresh_token(refresh_token: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(refresh_token.as_bytes());
    hex_encode(&digest.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut value, "{byte:02x}");
    }
    value
}
