use super::{
    Authenticated, AuthenticatedStreamableClient, ClientCore, ResourceCore, StreamableClient,
};
use crate::{
    Result, models,
    transport::{DefaultTransport, HttpTransport},
};
use std::{
    fmt,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    path::Path,
    sync::Arc,
};

/// A successful registration together with any generated credentials.
///
/// The value acts as the authenticated client, so client operations can be chained directly.
///
/// ```no_run
/// # async fn run() -> streamable::Result<()> {
/// let registration = streamable::StreamableClient::new()?
///     .register(None, None, None)
///     .await?;
/// println!("registered {}", registration.email());
/// # Ok(()) }
/// ```
pub struct Registration<T = DefaultTransport> {
    client: AuthenticatedStreamableClient<T>,
    email: String,
    password: String,
}

impl<T> fmt::Debug for Registration<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Registration")
            .finish_non_exhaustive()
    }
}

impl<T> Registration<T> {
    pub(super) const fn new(
        client: AuthenticatedStreamableClient<T>,
        email: String,
        password: String,
    ) -> Self {
        Self {
            client,
            email,
            password,
        }
    }

    /// Returns the registered email address.
    ///
    /// ```no_run
    /// # async fn run(registration: streamable::Registration) {
    /// println!("{}", registration.email());
    /// # }
    /// ```
    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    /// Returns the registered password.
    ///
    /// ```no_run
    /// # async fn run(registration: streamable::Registration) {
    /// let password = registration.password();
    /// # }
    /// ```
    #[must_use]
    pub fn password(&self) -> &str {
        &self.password
    }

    /// Returns the authenticated client.
    ///
    /// ```no_run
    /// # fn run(registration: &streamable::Registration) {
    /// assert!(registration.client().is_authenticated());
    /// # }
    /// ```
    #[must_use]
    pub const fn client(&self) -> &AuthenticatedStreamableClient<T> {
        &self.client
    }

    /// Signs out and returns an unauthenticated client.
    ///
    /// ```no_run
    /// # fn run(registration: streamable::Registration) {
    /// let client = registration.logout();
    /// assert!(!client.is_authenticated());
    /// # }
    /// ```
    #[must_use]
    pub fn logout(self) -> super::UnauthenticatedStreamableClient<T>
    where
        T: HttpTransport,
    {
        self.client.logout()
    }
}

impl<T> Deref for Registration<T> {
    type Target = AuthenticatedStreamableClient<T>;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl<T> DerefMut for Registration<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

/// A video bound to the client session that produced it.
///
/// ```no_run
/// # async fn run(client: streamable::UnauthenticatedStreamableClient) -> streamable::Result<()> {
/// let video = client.get_video("abc123").await?;
/// video.delete().await?;
/// # Ok(()) }
/// ```
pub struct Video<State = super::Unauthenticated, T = DefaultTransport> {
    pub(super) core: ResourceCore<T>,
    data: models::Video,
    state: PhantomData<State>,
}

impl<State, T> fmt::Debug for Video<State, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Video")
            .field("shortcode", &self.data.shortcode)
            .field("status", &self.data.status)
            .field("percent", &self.data.percent)
            .finish_non_exhaustive()
    }
}

impl<State, T> Video<State, T> {
    pub(super) const fn new(core: ResourceCore<T>, data: models::Video) -> Self {
        Self {
            core,
            data,
            state: PhantomData,
        }
    }

    /// Returns the latest stored wire snapshot.
    ///
    /// ```no_run
    /// # fn run(video: &streamable::Video) {
    /// println!("{}", video.data().shortcode);
    /// # }
    /// ```
    #[must_use]
    pub const fn data(&self) -> &models::Video {
        &self.data
    }

    /// Removes the session binding and returns the wire snapshot.
    ///
    /// ```no_run
    /// # fn run(video: streamable::Video) {
    /// let data = video.into_data();
    /// # }
    /// ```
    #[must_use]
    pub fn into_data(self) -> models::Video {
        self.data
    }
}

impl<State: Sync, T: HttpTransport> Video<State, T> {
    /// Refreshes this video from Streamable.
    ///
    /// ```no_run
    /// # async fn run(video: &mut streamable::Video) -> streamable::Result<()> {
    /// video.refresh().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response is not a video.
    pub async fn refresh(&mut self) -> Result<&models::Video> {
        self.data = self
            .core
            .execute(&models::GetVideoRequest::new(&self.data.shortcode))
            .await?;
        Ok(&self.data)
    }

    /// Deletes this video.
    ///
    /// ```no_run
    /// # async fn run(video: &streamable::Video) -> streamable::Result<()> {
    /// video.delete().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the deletion or the request fails.
    pub async fn delete(&self) -> Result<()> {
        self.core
            .execute(&models::DeleteVideoRequest::new(&self.data.shortcode))
            .await
    }

    /// Gets this video's analytics summary.
    ///
    /// ```no_run
    /// # async fn run(video: &streamable::Video) -> streamable::Result<()> {
    /// let analytics = video.analytics().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response cannot be decoded.
    pub async fn analytics(&self) -> Result<models::VideoAnalyticsSummary> {
        self.core
            .execute(&models::GetVideoAnalyticsRequest::new(&self.data.shortcode))
            .await
    }

    /// Gets this video's current live view count.
    ///
    /// ```no_run
    /// # async fn run(video: &streamable::Video) -> streamable::Result<()> {
    /// println!("{}", video.live_views().await?.count);
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails or the response cannot be decoded.
    pub async fn live_views(&self) -> Result<models::VideoLiveViews> {
        self.core
            .execute(&models::GetVideoLiveViewsRequest::new(&self.data.shortcode))
            .await
    }

    /// Changes this video's supplied privacy fields.
    ///
    /// ```no_run
    /// # async fn run(video: &streamable::Video) -> streamable::Result<()> {
    /// video.update_privacy(&Default::default()).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the settings or the request fails.
    pub async fn update_privacy(
        &self,
        settings: &models::VideoPrivacySettingsUpdate,
    ) -> Result<()> {
        self.core
            .execute(&models::UpdateVideoPrivacyRequest::new(
                &self.data.shortcode,
                settings,
            ))
            .await
    }

    /// Restores this video's default privacy settings.
    ///
    /// ```no_run
    /// # async fn run(video: &streamable::Video) -> streamable::Result<()> {
    /// video.reset_privacy().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the reset or the request fails.
    pub async fn reset_privacy(&self) -> Result<()> {
        self.core
            .execute(&models::ResetVideoPrivacyRequest::new(&self.data.shortcode))
            .await
    }

    /// Uses a video frame as this video's thumbnail and stores the returned snapshot.
    ///
    /// ```no_run
    /// # async fn run(video: &mut streamable::Video) -> streamable::Result<()> {
    /// video.set_thumbnail_frame(12.5).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid offset, a failed request, or an invalid response.
    pub async fn set_thumbnail_frame(&mut self, seconds: f64) -> Result<&models::Video> {
        self.core.ensure_valid()?;
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(crate::StreamableError::InvalidThumbnailOffset { seconds });
        }

        self.data = self
            .core
            .execute(&models::SetVideoThumbnailFrameRequest::new(
                &self.data.shortcode,
                seconds,
            ))
            .await?;
        Ok(&self.data)
    }

    /// Uploads an image as this video's thumbnail and stores the returned snapshot.
    ///
    /// ```no_run
    /// # async fn run(video: &mut streamable::Video) -> streamable::Result<()> {
    /// video.upload_thumbnail("thumbnail.png").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid image, a failed request, or an invalid response.
    pub async fn upload_thumbnail(
        &mut self,
        image_file: impl AsRef<Path>,
    ) -> Result<&models::Video> {
        self.data =
            super::upload_video_thumbnail(&self.core, &self.data.shortcode, image_file.as_ref())
                .await?;
        Ok(&self.data)
    }
}

impl<T: HttpTransport> Video<Authenticated, T> {
    /// Replaces this video's labels in the supplied order.
    ///
    /// ```no_run
    /// # async fn run(video: &streamable::Video<streamable::Authenticated>) -> streamable::Result<()> {
    /// video.set_labels(&[3, 1]).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the assignment or the request fails.
    pub async fn set_labels(&self, label_ids: &[u64]) -> Result<()> {
        self.core
            .execute(&models::SetVideoLabelsRequest::new(
                &self.data.shortcode,
                label_ids,
            ))
            .await
    }

    /// Removes every label from this video.
    ///
    /// ```no_run
    /// # async fn run(video: &mut streamable::Video<streamable::Authenticated>) -> streamable::Result<()> {
    /// video.clear_labels().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the assignment or the request fails.
    pub async fn clear_labels(&mut self) -> Result<()> {
        self.core
            .execute(&models::SetVideoLabelsRequest::new(
                &self.data.shortcode,
                &[],
            ))
            .await?;
        self.data.labels.clear();
        Ok(())
    }

    /// Removes the supplied labels from this video.
    ///
    /// Label identifiers that are not assigned to the video are ignored.
    ///
    /// ```no_run
    /// # async fn run(video: &mut streamable::Video<streamable::Authenticated>) -> streamable::Result<()> {
    /// video.remove_labels(&[3, 1]).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the video cannot be refreshed, Streamable rejects the assignment,
    /// or a request fails.
    pub async fn remove_labels(&mut self, label_ids: &[u64]) -> Result<()> {
        self.core.ensure_valid()?;
        if label_ids.is_empty() {
            return Ok(());
        }

        self.refresh().await?;
        let remaining_ids = self
            .data
            .labels
            .iter()
            .filter(|label| !label_ids.contains(&label.id))
            .map(|label| label.id)
            .collect::<Vec<_>>();

        if remaining_ids.len() == self.data.labels.len() {
            return Ok(());
        }

        self.core
            .execute(&models::SetVideoLabelsRequest::new(
                &self.data.shortcode,
                &remaining_ids,
            ))
            .await?;
        self.data
            .labels
            .retain(|label| !label_ids.contains(&label.id));
        Ok(())
    }
}

impl<State, T> Deref for Video<State, T> {
    type Target = models::Video;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// An account label bound to its authenticated client session.
///
/// ```no_run
/// # async fn run(client: streamable::AuthenticatedStreamableClient) -> streamable::Result<()> {
/// let label = client.create_label("reviewed").await?;
/// label.delete().await?;
/// # Ok(()) }
/// ```
pub struct Label<T = DefaultTransport> {
    core: ResourceCore<T>,
    data: models::Label,
}

impl<T> fmt::Debug for Label<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Label")
            .field("id", &self.data.id)
            .finish_non_exhaustive()
    }
}

impl<T> Label<T> {
    pub(super) const fn new(core: ResourceCore<T>, data: models::Label) -> Self {
        Self { core, data }
    }

    /// Returns the latest stored wire snapshot.
    ///
    /// ```no_run
    /// # fn run(label: &streamable::Label) {
    /// println!("{}", label.data().name);
    /// # }
    /// ```
    #[must_use]
    pub const fn data(&self) -> &models::Label {
        &self.data
    }

    /// Removes the session binding and returns the wire snapshot.
    ///
    /// ```no_run
    /// # fn run(label: streamable::Label) {
    /// let data = label.into_data();
    /// # }
    /// ```
    #[must_use]
    pub fn into_data(self) -> models::Label {
        self.data
    }
}

impl<T: HttpTransport> Label<T> {
    /// Renames this label and stores the returned snapshot.
    ///
    /// ```no_run
    /// # async fn run(label: &mut streamable::Label) -> streamable::Result<()> {
    /// label.rename("done").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the name or the request fails.
    pub async fn rename(&mut self, new_name: &str) -> Result<&models::Label> {
        self.data = self
            .core
            .execute(&models::RenameLabelRequest::new(self.data.id, new_name))
            .await?;
        Ok(&self.data)
    }

    /// Deletes this label.
    ///
    /// ```no_run
    /// # async fn run(label: &streamable::Label) -> streamable::Result<()> {
    /// label.delete().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the deletion or the request fails.
    pub async fn delete(&self) -> Result<()> {
        self.core
            .execute(&models::DeleteLabelRequest::new(self.data.id))
            .await
    }
}

impl<T> Deref for Label<T> {
    type Target = models::Label;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// A collection response bound to the client session that produced it.
///
/// `D` preserves the endpoint's exact collection response shape.
///
/// ```no_run
/// # async fn run(client: streamable::UnauthenticatedStreamableClient) -> streamable::Result<()> {
/// let collection = client.get_collection("shared1").await?;
/// collection.delete().await?;
/// # Ok(()) }
/// ```
pub struct Collection<State = super::Unauthenticated, T = DefaultTransport, D = models::Collection>
{
    core: Arc<ClientCore<T>>,
    shortcode: String,
    data: D,
    state: PhantomData<State>,
}

impl<State, T, D> fmt::Debug for Collection<State, T, D> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Collection")
            .field("shortcode", &self.shortcode)
            .finish_non_exhaustive()
    }
}

/// A collection returned by the paginated collection list.
///
/// ```no_run
/// # fn run(collection: &streamable::CollectionSummary) {
/// println!("{}", collection.shortcode());
/// # }
/// ```
pub type CollectionSummary<State = super::Unauthenticated, T = DefaultTransport> =
    Collection<State, T, models::CollectionSummary>;

/// A collection returned by a detail request.
///
/// ```no_run
/// # fn run(collection: &streamable::CollectionDetails) {
/// println!("{} videos", collection.videos.len());
/// # }
/// ```
pub type CollectionDetails<State = super::Unauthenticated, T = DefaultTransport> =
    Collection<State, T, models::CollectionDetails>;

impl<State, T, D> Collection<State, T, D> {
    pub(super) const fn new(core: Arc<ClientCore<T>>, shortcode: String, data: D) -> Self {
        Self {
            core,
            shortcode,
            data,
            state: PhantomData,
        }
    }

    /// Returns the collection shortcode.
    ///
    /// ```no_run
    /// # fn run(collection: &streamable::Collection) {
    /// println!("{}", collection.shortcode());
    /// # }
    /// ```
    #[must_use]
    pub fn shortcode(&self) -> &str {
        &self.shortcode
    }

    /// Returns the endpoint's exact stored wire snapshot.
    ///
    /// ```no_run
    /// # fn run(collection: &streamable::Collection) {
    /// println!("{:?}", collection.data().title);
    /// # }
    /// ```
    #[must_use]
    pub const fn data(&self) -> &D {
        &self.data
    }

    /// Removes the session binding and returns the wire snapshot.
    ///
    /// ```no_run
    /// # fn run(collection: streamable::Collection) {
    /// let data = collection.into_data();
    /// # }
    /// ```
    #[must_use]
    pub fn into_data(self) -> D {
        self.data
    }
}

impl<State: Sync, T: HttpTransport, D: Sync> Collection<State, T, D> {
    /// Gets the current detailed collection representation.
    ///
    /// ```no_run
    /// # async fn run(collection: &streamable::Collection) -> streamable::Result<()> {
    /// let details = collection.refresh().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when the collection cannot be fetched or decoded.
    pub async fn refresh(&self) -> Result<CollectionDetails<State, T>> {
        let data = self
            .core
            .execute(&models::GetCollectionRequest::new(&self.shortcode))
            .await?;
        Ok(Collection::new(
            Arc::clone(&self.core),
            self.shortcode.clone(),
            data,
        ))
    }

    /// Replaces the collection title and returns the updated collection snapshot.
    ///
    /// ```no_run
    /// # async fn run(collection: &streamable::Collection) -> streamable::Result<()> {
    /// let collection = collection.set_title("Highlights").await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the title or the request fails.
    pub async fn set_title(&self, title: &str) -> Result<Collection<State, T>> {
        let data = self
            .core
            .execute(&models::UpdateCollectionTitleRequest::new(
                &self.shortcode,
                title,
            ))
            .await?;
        Ok(Collection::new(
            Arc::clone(&self.core),
            self.shortcode.clone(),
            data,
        ))
    }

    /// Replaces the collection's complete video membership.
    ///
    /// ```no_run
    /// # async fn run(collection: &streamable::Collection) -> streamable::Result<()> {
    /// let collection = collection.replace_videos(&[]).await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the membership or the request fails.
    pub async fn replace_videos(&self, shortcodes: &[String]) -> Result<Collection<State, T>> {
        let data = self
            .core
            .execute(&models::ReplaceCollectionVideosRequest::new(
                &self.shortcode,
                shortcodes,
            ))
            .await?;
        Ok(Collection::new(
            Arc::clone(&self.core),
            self.shortcode.clone(),
            data,
        ))
    }

    /// Deletes this collection without deleting its member videos.
    ///
    /// ```no_run
    /// # async fn run(collection: &streamable::Collection) -> streamable::Result<()> {
    /// collection.delete().await?;
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error when Streamable rejects the deletion or the request fails.
    pub async fn delete(&self) -> Result<()> {
        self.core
            .execute(&models::DeleteCollectionRequest::new(&self.shortcode))
            .await
    }
}

impl<State, T, D> Deref for Collection<State, T, D> {
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

/// One page of client-bound collection summaries.
///
/// ```no_run
/// # async fn run(client: streamable::UnauthenticatedStreamableClient) -> streamable::Result<()> {
/// let page = client.list_collections(None, None).await?;
/// for collection in page.collections() {
///     println!("{}", collection.shortcode());
/// }
/// # Ok(()) }
/// ```
pub struct CollectionPage<State = super::Unauthenticated, T = DefaultTransport> {
    /// Collection summaries in service order.
    pub collections: Vec<CollectionSummary<State, T>>,
}

impl<State, T> fmt::Debug for CollectionPage<State, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollectionPage")
            .field("len", &self.collections.len())
            .finish()
    }
}

impl<State, T> CollectionPage<State, T> {
    pub(super) const fn new(collections: Vec<CollectionSummary<State, T>>) -> Self {
        Self { collections }
    }

    /// Returns the collection summaries in service order.
    ///
    /// ```no_run
    /// # fn run(page: &streamable::CollectionPage) {
    /// println!("{}", page.collections().len());
    /// # }
    /// ```
    #[must_use]
    pub fn collections(&self) -> &[CollectionSummary<State, T>] {
        &self.collections
    }

    /// Returns the client-bound collection summaries in service order.
    ///
    /// ```no_run
    /// # fn run(page: streamable::CollectionPage) {
    /// let collections = page.into_collections();
    /// # }
    /// ```
    #[must_use]
    pub fn into_collections(self) -> Vec<CollectionSummary<State, T>> {
        self.collections
    }
}

impl<T> StreamableClient<Authenticated, T> {
    pub(super) fn bind_label(&self, data: models::Label) -> Label<T> {
        Label::new(ResourceCore::new(Arc::clone(&self.core)), data)
    }
}
