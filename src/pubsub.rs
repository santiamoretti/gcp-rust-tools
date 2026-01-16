use std::sync::Arc;

use crate::helpers::gcp_config;
use google_cloud_auth::credentials::CredentialsFile;
use google_cloud_googleapis::pubsub::v1::PubsubMessage;
use google_cloud_pubsub::client::{Client, ClientConfig};
use google_cloud_pubsub::publisher::Publisher;
use google_cloud_pubsub::subscription::{Subscription, SubscriptionConfig};

use log::{debug, error, info};
use serde::Serialize;

pub struct PubSubsStuff {
    pub publishers: Arc<[(String, Publisher)]>,
    pub subscriptions: Arc<[(String, Subscription)]>,
}

impl PubSubsStuff {
    pub async fn new(
        project_id: Option<String>,
        instance_id: &str,
        topics: Arc<[&'static str]>,
        subs: Arc<[&'static str]>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("Initializing PubSub client");

        let key_file_path = gcp_config::credentials_path_from_env().map_err(|e| {
            let err: Box<dyn std::error::Error + Send + Sync> = e.into();
            err
        })?;

        let project_id = gcp_config::resolve_project_id(project_id)
            .await
            .map_err(|e| {
                let err: Box<dyn std::error::Error + Send + Sync> = e.into();
                err
            })?;

        info!("Using project_id: '{}'", project_id);

        // Expand topic names into full topic paths
        let expanded_topics: Vec<(String, &str)> = topics
            .iter()
            .map(|name| {
                (
                    format!("projects/{}/topics/{}-{}", project_id, name, instance_id),
                    *name,
                )
            })
            .collect();

        // Expand subscription names into full subscription paths
        let expanded_subs: Vec<(String, &str)> = subs
            .iter()
            .map(|name| {
                (
                    format!("projects/{}/subscriptions/{}", project_id, name),
                    *name,
                )
            })
            .collect();

        let credentials = CredentialsFile::new_from_file(key_file_path).await?;
        let config = ClientConfig::default()
            .with_credentials(credentials)
            .await?;
        let client = Client::new(config).await?;

        /* ---------- Publishers (build → freeze) ---------- */

        let mut publishers_vec = Vec::with_capacity(expanded_topics.len());

        for (topic_path, name) in expanded_topics.iter() {
            let publisher = client.topic(topic_path).new_publisher(None);
            publishers_vec.push((name.to_string(), publisher));
            debug!("Created publisher '{}'", name);
        }

        let publishers: Arc<[(String, Publisher)]> = Arc::from(publishers_vec);

        /* ---------- Subscriptions (build → freeze) ---------- */

        let mut subscriptions_vec = Vec::with_capacity(expanded_subs.len());

        for (sub_path, name) in expanded_subs.iter() {
            let sub_config = SubscriptionConfig {
                push_config: None,
                ack_deadline_seconds: 10,
                retain_acked_messages: false,
                message_retention_duration: None,
                labels: Default::default(),
                enable_message_ordering: true,
                expiration_policy: None,
                filter: String::new(),
                dead_letter_policy: None,
                retry_policy: None,
                detached: false,
                topic_message_retention_duration: None,
                enable_exactly_once_delivery: false,
                bigquery_config: None,
                state: 0,
                cloud_storage_config: None,
            };

            let subscription = match client
                .create_subscription(sub_path, "", sub_config, None)
                .await
            {
                Ok(sub) => sub,
                Err(err) => {
                    error!(
                        "Failed to create subscription '{}': {:?}. Falling back.",
                        name, err
                    );
                    client.subscription(sub_path)
                }
            };

            subscriptions_vec.push((name.to_string(), subscription));
            debug!("Created subscription '{}'", name);
        }

        let subscriptions: Arc<[(String, Subscription)]> = Arc::from(subscriptions_vec);

        info!("PubSub client initialized successfully");

        Ok(Self {
            publishers,
            subscriptions,
        })
    }

    /* ---------- Lookups ---------- */

    pub fn get_publisher(&self, name: &str) -> Option<Publisher> {
        self.publishers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p.clone())
    }

    pub fn get_subscription(&self, name: &str) -> Option<Subscription> {
        self.subscriptions
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s.clone())
    }

    /* ---------- Message helpers ---------- */

    pub fn create_message<T: Serialize>(
        &self,
        payload: T,
        ordering_key: Option<String>,
    ) -> Result<PubsubMessage, serde_json::Error> {
        let data = serde_json::to_vec(&payload)?;

        Ok(PubsubMessage {
            data,
            attributes: Default::default(),
            ordering_key: ordering_key.unwrap_or_default(),
            message_id: String::new(),
            publish_time: None,
        })
    }

    pub async fn publish_fire_and_forget<T: Serialize + Send + 'static>(
        &self,
        topic: &str,
        payload: T,
        ordering_key: Option<String>,
    ) {
        let publisher = self.get_publisher(topic);
        let topic_name = topic.to_string();

        tokio::spawn(async move {
            match publisher {
                Some(publisher) => match serde_json::to_vec(&payload) {
                    Ok(data) => {
                        let message = PubsubMessage {
                            data,
                            attributes: Default::default(),
                            ordering_key: ordering_key.unwrap_or_default(),
                            message_id: String::new(),
                            publish_time: None,
                        };
                        publisher.publish(message).await;
                        debug!("Message published to '{}'", topic_name);
                    }
                    Err(e) => error!("Failed to serialize payload: {:?}", e),
                },
                None => error!("Publisher '{}' not found", topic_name),
            }
        });
    }
}

pub async fn create_pubsub_client(
    project_id: Option<String>,
    instance_id: &str,
    topics: Arc<[&'static str]>,
    subs: Arc<[&'static str]>,
) -> Result<PubSubsStuff, Box<dyn std::error::Error + Send + Sync>> {
    PubSubsStuff::new(project_id, instance_id, topics, subs).await
}
