use std::{collections::HashMap, sync::RwLock};

use anneal_core::{Actor, ApplicationError, ApplicationResult, UserStatus};
use anneal_users::{User, UserRepository};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::domain::{
    AccessClaims, LoginResult, PreAuthChallenge, PreAuthPurpose, RefreshSession, SessionContext,
    SessionTokens, TotpSetup,
};

#[async_trait]
pub trait PasswordService: Send + Sync {
    async fn hash_password(&self, password: &str) -> ApplicationResult<String>;
    async fn verify_password(&self, password: &str, password_hash: &str)
    -> ApplicationResult<bool>;
}

#[async_trait]
impl<T> PasswordService for &T
where
    T: PasswordService + Send + Sync,
{
    async fn hash_password(&self, password: &str) -> ApplicationResult<String> {
        (*self).hash_password(password).await
    }

    async fn verify_password(
        &self,
        password: &str,
        password_hash: &str,
    ) -> ApplicationResult<bool> {
        (*self).verify_password(password, password_hash).await
    }
}

pub trait AccessTokenService: Send + Sync {
    fn issue_access_token(
        &self,
        actor: &Actor,
    ) -> ApplicationResult<(String, chrono::DateTime<Utc>)>;
    fn issue_pre_auth_token(
        &self,
        actor: &Actor,
        challenge_id: Uuid,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<String>;
    fn decode_claims(&self, token: &str) -> ApplicationResult<AccessClaims>;
}

impl<T> AccessTokenService for &T
where
    T: AccessTokenService + Send + Sync,
{
    fn issue_access_token(
        &self,
        actor: &Actor,
    ) -> ApplicationResult<(String, chrono::DateTime<Utc>)> {
        (*self).issue_access_token(actor)
    }

    fn issue_pre_auth_token(
        &self,
        actor: &Actor,
        challenge_id: Uuid,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<String> {
        (*self).issue_pre_auth_token(actor, challenge_id, purpose)
    }

    fn decode_claims(&self, token: &str) -> ApplicationResult<AccessClaims> {
        (*self).decode_claims(token)
    }
}

pub trait TotpService: Send + Sync {
    fn generate(&self, email: &str) -> ApplicationResult<TotpSetup>;
    fn build(&self, secret: &str, email: &str) -> ApplicationResult<TotpSetup>;
    fn verify(&self, secret: &str, code: &str, email: &str) -> ApplicationResult<bool>;
}

impl<T> TotpService for &T
where
    T: TotpService + Send + Sync,
{
    fn generate(&self, email: &str) -> ApplicationResult<TotpSetup> {
        (*self).generate(email)
    }

    fn build(&self, secret: &str, email: &str) -> ApplicationResult<TotpSetup> {
        (*self).build(secret, email)
    }

    fn verify(&self, secret: &str, code: &str, email: &str) -> ApplicationResult<bool> {
        (*self).verify(secret, code, email)
    }
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create_session(&self, session: RefreshSession) -> ApplicationResult<RefreshSession>;
    async fn consume_active_session_by_hash(
        &self,
        refresh_token_hash: &str,
    ) -> ApplicationResult<Option<RefreshSession>>;
    async fn find_active_session_by_hash(
        &self,
        refresh_token_hash: &str,
    ) -> ApplicationResult<Option<RefreshSession>>;
    async fn revoke_session(&self, session_id: Uuid) -> ApplicationResult<()>;
    async fn list_user_sessions(&self, user_id: Uuid) -> ApplicationResult<Vec<RefreshSession>>;
    async fn revoke_user_sessions(&self, user_id: Uuid) -> ApplicationResult<()>;
    async fn create_pre_auth_challenge(
        &self,
        challenge: PreAuthChallenge,
    ) -> ApplicationResult<PreAuthChallenge>;
    async fn find_active_pre_auth_challenge(
        &self,
        challenge_id: Uuid,
    ) -> ApplicationResult<Option<PreAuthChallenge>>;
    async fn update_pre_auth_challenge_secret(
        &self,
        challenge_id: Uuid,
        pending_totp_secret: &str,
    ) -> ApplicationResult<()>;
    async fn consume_pre_auth_challenge(
        &self,
        challenge_id: Uuid,
        user_id: Uuid,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<Option<PreAuthChallenge>>;
}

#[async_trait]
impl<T> SessionRepository for &T
where
    T: SessionRepository + Send + Sync,
{
    async fn create_session(&self, session: RefreshSession) -> ApplicationResult<RefreshSession> {
        (*self).create_session(session).await
    }

    async fn consume_active_session_by_hash(
        &self,
        refresh_token_hash: &str,
    ) -> ApplicationResult<Option<RefreshSession>> {
        (*self)
            .consume_active_session_by_hash(refresh_token_hash)
            .await
    }

    async fn find_active_session_by_hash(
        &self,
        refresh_token_hash: &str,
    ) -> ApplicationResult<Option<RefreshSession>> {
        (*self)
            .find_active_session_by_hash(refresh_token_hash)
            .await
    }

    async fn revoke_session(&self, session_id: Uuid) -> ApplicationResult<()> {
        (*self).revoke_session(session_id).await
    }

    async fn list_user_sessions(&self, user_id: Uuid) -> ApplicationResult<Vec<RefreshSession>> {
        (*self).list_user_sessions(user_id).await
    }

    async fn revoke_user_sessions(&self, user_id: Uuid) -> ApplicationResult<()> {
        (*self).revoke_user_sessions(user_id).await
    }

    async fn create_pre_auth_challenge(
        &self,
        challenge: PreAuthChallenge,
    ) -> ApplicationResult<PreAuthChallenge> {
        (*self).create_pre_auth_challenge(challenge).await
    }

    async fn find_active_pre_auth_challenge(
        &self,
        challenge_id: Uuid,
    ) -> ApplicationResult<Option<PreAuthChallenge>> {
        (*self).find_active_pre_auth_challenge(challenge_id).await
    }

    async fn update_pre_auth_challenge_secret(
        &self,
        challenge_id: Uuid,
        pending_totp_secret: &str,
    ) -> ApplicationResult<()> {
        (*self)
            .update_pre_auth_challenge_secret(challenge_id, pending_totp_secret)
            .await
    }

    async fn consume_pre_auth_challenge(
        &self,
        challenge_id: Uuid,
        user_id: Uuid,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<Option<PreAuthChallenge>> {
        (*self)
            .consume_pre_auth_challenge(challenge_id, user_id, purpose)
            .await
    }
}

pub struct AuthService<U, S, P, T, A> {
    users: U,
    sessions: S,
    passwords: P,
    totp: T,
    access_tokens: A,
}

impl<U, S, P, T, A> AuthService<U, S, P, T, A> {
    pub fn new(users: U, sessions: S, passwords: P, totp: T, access_tokens: A) -> Self {
        Self {
            users,
            sessions,
            passwords,
            totp,
            access_tokens,
        }
    }
}

impl<U, S, P, T, A> AuthService<U, S, P, T, A>
where
    U: UserRepository,
    S: SessionRepository,
    P: PasswordService,
    T: TotpService,
    A: AccessTokenService,
{
    pub async fn hash_password(&self, password: &str) -> ApplicationResult<String> {
        self.passwords.hash_password(password).await
    }

    pub async fn login(
        &self,
        email: &str,
        password: &str,
        totp_code: Option<&str>,
        session_context: SessionContext,
    ) -> ApplicationResult<LoginResult> {
        let user = self
            .users
            .get_user_by_email(email)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if !self
            .passwords
            .verify_password(password, &user.password_hash)
            .await?
        {
            return Err(ApplicationError::Unauthorized);
        }
        if user.status != UserStatus::Active {
            return Err(ApplicationError::Unauthorized);
        }
        let actor = Self::actor_from_user(&user);
        if user.role.is_staff() && !user.totp_confirmed {
            let challenge = self
                .issue_pre_auth_challenge(&user, PreAuthPurpose::TotpSetup)
                .await?;
            return Ok(LoginResult::TotpSetupRequired {
                pre_auth_token: self.access_tokens.issue_pre_auth_token(
                    &actor,
                    challenge.id,
                    PreAuthPurpose::TotpSetup,
                )?,
            });
        }
        if let Some(secret) = user.totp_secret.as_ref() {
            let code = match totp_code {
                Some(code) => code,
                None => {
                    let challenge = self
                        .issue_pre_auth_challenge(&user, PreAuthPurpose::TotpVerify)
                        .await?;
                    return Ok(LoginResult::TotpRequired {
                        pre_auth_token: self.access_tokens.issue_pre_auth_token(
                            &actor,
                            challenge.id,
                            PreAuthPurpose::TotpVerify,
                        )?,
                    });
                }
            };
            if !self.totp.verify(secret, code, &user.email)? {
                return Err(ApplicationError::Unauthorized);
            }
        }
        let tokens = self.issue_session(&user, session_context, None).await?;
        Ok(LoginResult::Authenticated { tokens })
    }

    pub async fn begin_totp_setup(&self, claims: &AccessClaims) -> ApplicationResult<TotpSetup> {
        let challenge = self
            .load_active_challenge(claims, PreAuthPurpose::TotpSetup)
            .await?;
        let user = self
            .users
            .get_user_by_id(claims.sub)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if let Some(secret) = challenge.pending_totp_secret {
            return self.totp.build(&secret, &user.email);
        }
        let setup = self.totp.generate(&user.email)?;
        self.sessions
            .update_pre_auth_challenge_secret(challenge.id, &setup.secret)
            .await?;
        Ok(setup)
    }

    pub async fn verify_totp(
        &self,
        claims: &AccessClaims,
        code: &str,
        session_context: SessionContext,
    ) -> ApplicationResult<SessionTokens> {
        let purpose = claims
            .purpose
            .as_deref()
            .ok_or(ApplicationError::Unauthorized)?;
        let purpose = match purpose {
            "totp_setup" => PreAuthPurpose::TotpSetup,
            "totp_verify" => PreAuthPurpose::TotpVerify,
            _ => return Err(ApplicationError::Unauthorized),
        };
        let challenge = self
            .consume_active_challenge(claims, purpose)
            .await?;
        let user = self
            .users
            .get_user_by_id(claims.sub)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        let secret = match purpose {
            PreAuthPurpose::TotpSetup => challenge
                .pending_totp_secret
                .as_ref()
                .ok_or(ApplicationError::Unauthorized)?,
            PreAuthPurpose::TotpVerify => user
                .totp_secret
                .as_ref()
                .ok_or(ApplicationError::Unauthorized)?,
        };
        if !self.totp.verify(secret, code, &user.email)? {
            return Err(ApplicationError::Unauthorized);
        }
        if purpose == PreAuthPurpose::TotpSetup {
            self.users.save_totp_secret(user.id, secret).await?;
        }
        self.users.confirm_totp(user.id).await?;
        let refreshed = self
            .users
            .get_user_by_id(user.id)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        self.issue_session(&refreshed, session_context, None).await
    }

    pub async fn disable_totp(&self, actor: &Actor, password: &str) -> ApplicationResult<()> {
        let user = self
            .users
            .get_user_by_id(actor.user_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("user not found".into()))?;
        if !self
            .passwords
            .verify_password(password, &user.password_hash)
            .await?
        {
            return Err(ApplicationError::Unauthorized);
        }
        self.users.clear_totp(user.id).await?;
        self.sessions.revoke_user_sessions(user.id).await
    }

    pub async fn refresh(
        &self,
        refresh_token: &str,
        session_context: SessionContext,
    ) -> ApplicationResult<SessionTokens> {
        let refresh_token_hash = crate::infrastructure::hash_refresh_token(refresh_token);
        let session = self
            .sessions
            .consume_active_session_by_hash(&refresh_token_hash)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if session.expires_at <= Utc::now() {
            return Err(ApplicationError::Unauthorized);
        }
        let user = self
            .users
            .get_user_by_id(session.user_id)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if user.status != UserStatus::Active {
            return Err(ApplicationError::Unauthorized);
        }
        self.issue_session(&user, session_context, Some(session.id))
            .await
    }

    pub async fn logout(&self, refresh_token: &str) -> ApplicationResult<()> {
        let refresh_token_hash = crate::infrastructure::hash_refresh_token(refresh_token);
        if let Some(session) = self
            .sessions
            .find_active_session_by_hash(&refresh_token_hash)
            .await?
        {
            self.sessions.revoke_session(session.id).await?;
        }
        Ok(())
    }

    pub async fn logout_all(&self, actor: &Actor) -> ApplicationResult<()> {
        self.sessions.revoke_user_sessions(actor.user_id).await
    }

    pub async fn list_sessions(&self, actor: &Actor) -> ApplicationResult<Vec<RefreshSession>> {
        self.sessions.list_user_sessions(actor.user_id).await
    }

    pub async fn change_password(
        &self,
        actor: &Actor,
        current_password: &str,
        new_password: &str,
    ) -> ApplicationResult<()> {
        let user = self
            .users
            .get_user_by_id(actor.user_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound("user not found".into()))?;
        if !self
            .passwords
            .verify_password(current_password, &user.password_hash)
            .await?
        {
            return Err(ApplicationError::Unauthorized);
        }
        let password_hash = self.passwords.hash_password(new_password).await?;
        self.users
            .update_password_hash(user.id, &password_hash)
            .await?;
        self.sessions.revoke_user_sessions(user.id).await
    }

    pub fn decode_claims(&self, token: &str) -> ApplicationResult<AccessClaims> {
        self.access_tokens.decode_claims(token)
    }

    async fn issue_session(
        &self,
        user: &User,
        session_context: SessionContext,
        rotated_from_session_id: Option<Uuid>,
    ) -> ApplicationResult<SessionTokens> {
        let actor = Self::actor_from_user(user);
        let (access_token, access_expires_at) = self.access_tokens.issue_access_token(&actor)?;
        let refresh_token = crate::infrastructure::generate_refresh_token();
        let refresh_token_hash = crate::infrastructure::hash_refresh_token(&refresh_token);
        let refresh_expires_at = Utc::now() + Duration::days(30);
        let session = RefreshSession {
            id: Uuid::new_v4(),
            user_id: user.id,
            refresh_token_hash,
            user_agent: session_context.user_agent,
            ip_address: session_context.ip_address,
            expires_at: refresh_expires_at,
            revoked_at: None,
            rotated_from_session_id,
            created_at: Utc::now(),
        };
        self.sessions.create_session(session).await?;
        Ok(SessionTokens {
            access_token,
            refresh_token,
            access_expires_at,
            refresh_expires_at,
        })
    }

    fn actor_from_user(user: &User) -> Actor {
        Actor {
            user_id: user.id,
            tenant_id: user.tenant_id,
            role: user.role,
        }
    }

    async fn issue_pre_auth_challenge(
        &self,
        user: &User,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<PreAuthChallenge> {
        self.sessions
            .create_pre_auth_challenge(PreAuthChallenge {
                id: Uuid::new_v4(),
                user_id: user.id,
                purpose: purpose.as_str().into(),
                pending_totp_secret: None,
                expires_at: Utc::now() + Duration::minutes(10),
                used_at: None,
                created_at: Utc::now(),
            })
            .await
    }

    async fn load_active_challenge(
        &self,
        claims: &AccessClaims,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<PreAuthChallenge> {
        if claims.kind != "pre_auth" {
            return Err(ApplicationError::Unauthorized);
        }
        if claims.purpose.as_deref() != Some(purpose.as_str()) {
            return Err(ApplicationError::Unauthorized);
        }
        let challenge_id = claims.challenge_id.ok_or(ApplicationError::Unauthorized)?;
        let challenge = self
            .sessions
            .find_active_pre_auth_challenge(challenge_id)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if challenge.user_id != claims.sub
            || challenge.purpose != purpose.as_str()
            || challenge.used_at.is_some()
            || challenge.expires_at <= Utc::now()
        {
            return Err(ApplicationError::Unauthorized);
        }
        Ok(challenge)
    }

    async fn consume_active_challenge(
        &self,
        claims: &AccessClaims,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<PreAuthChallenge> {
        if claims.kind != "pre_auth" {
            return Err(ApplicationError::Unauthorized);
        }
        if claims.purpose.as_deref() != Some(purpose.as_str()) {
            return Err(ApplicationError::Unauthorized);
        }
        let challenge_id = claims.challenge_id.ok_or(ApplicationError::Unauthorized)?;
        let challenge = self
            .sessions
            .consume_pre_auth_challenge(challenge_id, claims.sub, purpose)
            .await?
            .ok_or(ApplicationError::Unauthorized)?;
        if challenge.expires_at <= Utc::now() {
            return Err(ApplicationError::Unauthorized);
        }
        Ok(challenge)
    }
}

#[derive(Default)]
pub struct InMemorySessionRepository {
    sessions: RwLock<HashMap<Uuid, RefreshSession>>,
    pre_auth_challenges: RwLock<HashMap<Uuid, PreAuthChallenge>>,
}

#[async_trait]
impl SessionRepository for InMemorySessionRepository {
    async fn create_session(&self, session: RefreshSession) -> ApplicationResult<RefreshSession> {
        self.sessions
            .write()
            .expect("lock")
            .insert(session.id, session.clone());
        Ok(session)
    }

    async fn consume_active_session_by_hash(
        &self,
        refresh_token_hash: &str,
    ) -> ApplicationResult<Option<RefreshSession>> {
        let mut sessions = self.sessions.write().expect("lock");
        let session_id = sessions.iter().find_map(|(session_id, session)| {
            (session.refresh_token_hash == refresh_token_hash
                && session.revoked_at.is_none()
                && session.expires_at > Utc::now())
            .then_some(*session_id)
        });
        let Some(session_id) = session_id else {
            return Ok(None);
        };
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| ApplicationError::NotFound("session not found".into()))?;
        session.revoked_at = Some(Utc::now());
        Ok(Some(session.clone()))
    }

    async fn find_active_session_by_hash(
        &self,
        refresh_token_hash: &str,
    ) -> ApplicationResult<Option<RefreshSession>> {
        Ok(self
            .sessions
            .read()
            .expect("lock")
            .values()
            .find(|session| {
                session.refresh_token_hash == refresh_token_hash && session.revoked_at.is_none()
            })
            .cloned())
    }

    async fn revoke_session(&self, session_id: Uuid) -> ApplicationResult<()> {
        let mut sessions = self.sessions.write().expect("lock");
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| ApplicationError::NotFound("session not found".into()))?;
        session.revoked_at = Some(Utc::now());
        Ok(())
    }

    async fn list_user_sessions(&self, user_id: Uuid) -> ApplicationResult<Vec<RefreshSession>> {
        Ok(self
            .sessions
            .read()
            .expect("lock")
            .values()
            .filter(|session| session.user_id == user_id)
            .cloned()
            .collect())
    }

    async fn revoke_user_sessions(&self, user_id: Uuid) -> ApplicationResult<()> {
        let mut sessions = self.sessions.write().expect("lock");
        for session in sessions
            .values_mut()
            .filter(|session| session.user_id == user_id)
        {
            session.revoked_at = Some(Utc::now());
        }
        Ok(())
    }

    async fn create_pre_auth_challenge(
        &self,
        challenge: PreAuthChallenge,
    ) -> ApplicationResult<PreAuthChallenge> {
        self.pre_auth_challenges
            .write()
            .expect("lock")
            .insert(challenge.id, challenge.clone());
        Ok(challenge)
    }

    async fn find_active_pre_auth_challenge(
        &self,
        challenge_id: Uuid,
    ) -> ApplicationResult<Option<PreAuthChallenge>> {
        Ok(self
            .pre_auth_challenges
            .read()
            .expect("lock")
            .get(&challenge_id)
            .cloned())
    }

    async fn update_pre_auth_challenge_secret(
        &self,
        challenge_id: Uuid,
        pending_totp_secret: &str,
    ) -> ApplicationResult<()> {
        let mut challenges = self.pre_auth_challenges.write().expect("lock");
        let challenge = challenges
            .get_mut(&challenge_id)
            .ok_or(ApplicationError::Unauthorized)?;
        challenge.pending_totp_secret = Some(pending_totp_secret.into());
        Ok(())
    }

    async fn consume_pre_auth_challenge(
        &self,
        challenge_id: Uuid,
        user_id: Uuid,
        purpose: PreAuthPurpose,
    ) -> ApplicationResult<Option<PreAuthChallenge>> {
        let mut challenges = self.pre_auth_challenges.write().expect("lock");
        let Some(challenge) = challenges.get_mut(&challenge_id) else {
            return Ok(None);
        };
        if challenge.user_id != user_id
            || challenge.purpose != purpose.as_str()
            || challenge.used_at.is_some()
            || challenge.expires_at <= Utc::now()
        {
            return Ok(None);
        }
        challenge.used_at = Some(Utc::now());
        Ok(Some(challenge.clone()))
    }
}

#[cfg(test)]
mod tests {
    use anneal_rbac::RbacService;
    use anneal_users::{InMemoryUserRepository, UserRepository, UserService};
    use chrono::Utc;

    use crate::{
        application::{
            AuthService, InMemorySessionRepository, PasswordService, SessionContext, TotpService,
        },
        domain::LoginResult,
        infrastructure::{ArgonPasswordService, JwtService},
    };

    struct FixedTotpService;

    impl TotpService for FixedTotpService {
        fn generate(
            &self,
            _email: &str,
        ) -> anneal_core::ApplicationResult<crate::domain::TotpSetup> {
            Ok(crate::domain::TotpSetup {
                secret: "SECRET".into(),
                otpauth_url: "otpauth://test".into(),
            })
        }

        fn build(
            &self,
            secret: &str,
            _email: &str,
        ) -> anneal_core::ApplicationResult<crate::domain::TotpSetup> {
            Ok(crate::domain::TotpSetup {
                secret: secret.into(),
                otpauth_url: "otpauth://test".into(),
            })
        }

        fn verify(
            &self,
            _secret: &str,
            code: &str,
            _email: &str,
        ) -> anneal_core::ApplicationResult<bool> {
            Ok(code == "123456")
        }
    }

    #[tokio::test]
    async fn refresh_rotates_session() {
        let users = InMemoryUserRepository::default();
        let user_service = UserService::new(users, RbacService);
        let password_service = ArgonPasswordService;
        let password_hash = password_service
            .hash_password("password")
            .await
            .expect("hash");
        let user = user_service
            .bootstrap_superadmin("admin@test.local".into(), "Admin".into(), password_hash)
            .await
            .expect("bootstrap");
        user_service
            .repository()
            .save_totp_secret(user.id, "SECRET")
            .await
            .expect("totp secret");
        user_service
            .repository()
            .confirm_totp(user.id)
            .await
            .expect("confirm totp");
        let auth = AuthService::new(
            user_service.repository(),
            InMemorySessionRepository::default(),
            ArgonPasswordService,
            FixedTotpService,
            JwtService::new("access", "preauth"),
        );
        let login = auth
            .login(
                "admin@test.local",
                "password",
                Some("123456"),
                SessionContext {
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await
            .expect("login");

        let tokens = match login {
            LoginResult::Authenticated { tokens } => tokens,
            _ => panic!("unexpected login result"),
        };

        let rotated = auth
            .refresh(
                &tokens.refresh_token,
                SessionContext {
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await
            .expect("refresh");

        assert_ne!(tokens.refresh_token, rotated.refresh_token);
    }

    #[tokio::test]
    async fn staff_requires_totp_setup_first() {
        let users = InMemoryUserRepository::default();
        let user_service = UserService::new(users, RbacService);
        let password_hash = ArgonPasswordService
            .hash_password("password")
            .await
            .expect("hash");
        let user = user_service
            .bootstrap_superadmin("admin@test.local".into(), "Admin".into(), password_hash)
            .await
            .expect("bootstrap");
        let auth = AuthService::new(
            user_service.repository(),
            InMemorySessionRepository::default(),
            ArgonPasswordService,
            FixedTotpService,
            JwtService::new("access", "preauth"),
        );
        let login = auth
            .login(
                "admin@test.local",
                "password",
                None,
                SessionContext {
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await
            .expect("login");

        let token = match login {
            LoginResult::TotpSetupRequired { pre_auth_token } => pre_auth_token,
            _ => panic!("unexpected login result"),
        };
        let claims = auth.decode_claims(&token).expect("claims");
        assert_eq!(claims.sub, user.id);
        assert_eq!(claims.kind, "pre_auth");
        assert_eq!(claims.purpose.as_deref(), Some("totp_setup"));
        assert!(claims.challenge_id.is_some());
        assert!(claims.exp > Utc::now().timestamp() as usize);
    }

    #[tokio::test]
    async fn access_token_cannot_open_pre_auth_flow() {
        let users = InMemoryUserRepository::default();
        let user_service = UserService::new(users, RbacService);
        let password_hash = ArgonPasswordService
            .hash_password("password")
            .await
            .expect("hash");
        let user = user_service
            .bootstrap_superadmin("admin@test.local".into(), "Admin".into(), password_hash)
            .await
            .expect("bootstrap");
        user_service
            .repository()
            .save_totp_secret(user.id, "SECRET")
            .await
            .expect("totp secret");
        user_service
            .repository()
            .confirm_totp(user.id)
            .await
            .expect("confirm");
        let auth = AuthService::new(
            user_service.repository(),
            InMemorySessionRepository::default(),
            ArgonPasswordService,
            FixedTotpService,
            JwtService::new("access", "preauth"),
        );
        let login = auth
            .login(
                "admin@test.local",
                "password",
                Some("123456"),
                SessionContext {
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await
            .expect("login");
        let tokens = match login {
            LoginResult::Authenticated { tokens } => tokens,
            _ => panic!("unexpected login result"),
        };
        let claims = auth.decode_claims(&tokens.access_token).expect("claims");
        let error = auth
            .begin_totp_setup(&claims)
            .await
            .expect_err("access token must fail");
        assert!(matches!(error, anneal_core::ApplicationError::Unauthorized));
    }

    #[tokio::test]
    async fn totp_setup_reuses_same_secret_within_challenge() {
        let users = InMemoryUserRepository::default();
        let user_service = UserService::new(users, RbacService);
        let password_hash = ArgonPasswordService
            .hash_password("password")
            .await
            .expect("hash");
        user_service
            .bootstrap_superadmin("admin@test.local".into(), "Admin".into(), password_hash)
            .await
            .expect("bootstrap");
        let auth = AuthService::new(
            user_service.repository(),
            InMemorySessionRepository::default(),
            ArgonPasswordService,
            FixedTotpService,
            JwtService::new("access", "preauth"),
        );
        let login = auth
            .login(
                "admin@test.local",
                "password",
                None,
                SessionContext {
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await
            .expect("login");
        let token = match login {
            LoginResult::TotpSetupRequired { pre_auth_token } => pre_auth_token,
            _ => panic!("unexpected login result"),
        };
        let claims = auth.decode_claims(&token).expect("claims");
        let first = auth.begin_totp_setup(&claims).await.expect("first setup");
        let second = auth.begin_totp_setup(&claims).await.expect("second setup");
        assert_eq!(first.secret, second.secret);
    }

    #[tokio::test]
    async fn suspended_user_gets_unauthorized_and_cannot_refresh() {
        let users = InMemoryUserRepository::default();
        let user_service = UserService::new(users, RbacService);
        let password_hash = ArgonPasswordService
            .hash_password("password")
            .await
            .expect("hash");
        let user = user_service
            .bootstrap_superadmin("admin@test.local".into(), "Admin".into(), password_hash)
            .await
            .expect("bootstrap");
        user_service
            .repository()
            .save_totp_secret(user.id, "SECRET")
            .await
            .expect("totp secret");
        user_service
            .repository()
            .confirm_totp(user.id)
            .await
            .expect("confirm");
        let auth = AuthService::new(
            user_service.repository(),
            InMemorySessionRepository::default(),
            ArgonPasswordService,
            FixedTotpService,
            JwtService::new("access", "preauth"),
        );
        let login = auth
            .login(
                "admin@test.local",
                "password",
                Some("123456"),
                SessionContext {
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await
            .expect("login");
        let tokens = match login {
            LoginResult::Authenticated { tokens } => tokens,
            _ => panic!("unexpected login result"),
        };
        let suspended = user_service
            .repository()
            .get_user_by_id(user.id)
            .await
            .expect("user lookup")
            .expect("user");
        user_service
            .repository()
            .update_user(anneal_users::User {
                status: anneal_core::UserStatus::Suspended,
                ..suspended
            })
            .await
            .expect("suspend");

        let login_error = auth
            .login(
                "admin@test.local",
                "password",
                Some("123456"),
                SessionContext {
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await
            .expect_err("suspended login must fail");
        assert!(matches!(login_error, anneal_core::ApplicationError::Unauthorized));

        let refresh_error = auth
            .refresh(
                &tokens.refresh_token,
                SessionContext {
                    user_agent: None,
                    ip_address: None,
                },
            )
            .await
            .expect_err("refresh must fail");
        assert!(matches!(refresh_error, anneal_core::ApplicationError::Unauthorized));
    }
}
