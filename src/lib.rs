// Copyright 2019. The Tari Project
//
// Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
// following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following
// disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
// following disclaimer in the documentation and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote
// products derived from this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
use futures::{future, prelude::*, stream::Fuse};
use std::fmt::Debug;
use tari_broadcast_channel::{bounded, Publisher, Subscriber};

/// The container for a message that is passed along the pub-sub channel that contains a Topic to define the type of
/// message and the message itself.
#[derive(Debug)]
pub struct TopicPayload<T, M> {
    topic: T,
    message: M,
}

impl<T, M> TopicPayload<T, M> {
    pub fn new(topic: T, message: M) -> Self {
        Self { topic, message }
    }

    pub fn topic(&self) -> &T {
        &self.topic
    }

    pub fn message(&self) -> &M {
        &self.message
    }
}

pub type TopicPublisher<T, M> = Publisher<TopicPayload<T, M>>;
pub type TopicSubscriber<T, M> = Subscriber<TopicPayload<T, M>>;

/// This structure hold the Subscriber end of a Pub-Sub channel and can be used to create new filtered subscriber
/// channels.
pub struct TopicSubscriptionFactory<T, M> {
    subscriber: TopicSubscriber<T, M>,
}

impl<T, M> TopicSubscriptionFactory<T, M>
where
    T: Eq + Send,
    M: Clone + Send,
{
    pub fn new(subscriber: TopicSubscriber<T, M>) -> Self {
        TopicSubscriptionFactory { subscriber }
    }

    /// Provide a subscriber (which will be consumed) and a topic to filter it by and this function will return a stream
    /// that yields only the desired messages
    pub fn get_subscription(&self, topic: T) -> impl Stream<Item = M> {
        self.subscriber.clone().filter_map(move |item| {
            let result = if item.topic() == &topic {
                Some(item.message.clone())
            } else {
                None
            };
            future::ready(result)
        })
    }

    /// Provide a fused version of the subscription stream so that domain modules don't need to know about fuse()
    pub fn get_subscription_fused(&self, topic: T) -> Fuse<impl Stream<Item = M>> {
        self.get_subscription(topic).fuse()
    }
}

/// Create Topic based Pub-Sub channel which returns the Publisher side of the channel and TopicSubscriptionFactory
/// which can produce multiple subscribers for provided topics. The initial receiver id is required and used to label
/// the subscribers, which makes debugging simpler.
pub fn pubsub_channel_with_id<T: Send + Eq, M: Send + Clone>(
    size: usize,
    receiver_id: usize,
) -> (TopicPublisher<T, M>, TopicSubscriptionFactory<T, M>)
{
    let (publisher, subscriber): (TopicPublisher<T, M>, TopicSubscriber<T, M>) = bounded(size, receiver_id);
    (publisher, TopicSubscriptionFactory::new(subscriber))
}

/// Create a topi-based pub-sub channel with a default receiver id of 1
pub fn pubsub_channel<T: Send + Eq, M: Send + Clone>(
    size: usize,
) -> (TopicPublisher<T, M>, TopicSubscriptionFactory<T, M>) {
    pubsub_channel_with_id(size, 1)
}

#[cfg(test)]
mod test {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn topic_pub_sub() {
        let (mut publisher, subscriber_factory) = pubsub_channel(10);

        #[derive(Debug, Clone)]
        struct Dummy {
            a: u32,
            b: String,
        }

        let messages = vec![
            TopicPayload::new("Topic1", Dummy {
                a: 1u32,
                b: "one".to_string(),
            }),
            TopicPayload::new("Topic2", Dummy {
                a: 2u32,
                b: "two".to_string(),
            }),
            TopicPayload::new("Topic1", Dummy {
                a: 3u32,
                b: "three".to_string(),
            }),
            TopicPayload::new("Topic2", Dummy {
                a: 4u32,
                b: "four".to_string(),
            }),
            TopicPayload::new("Topic1", Dummy {
                a: 5u32,
                b: "five".to_string(),
            }),
            TopicPayload::new("Topic2", Dummy {
                a: 6u32,
                b: "size".to_string(),
            }),
            TopicPayload::new("Topic1", Dummy {
                a: 7u32,
                b: "seven".to_string(),
            }),
        ];

        block_on(async {
            for m in messages {
                publisher.send(m).await.unwrap();
            }
        });

        let mut sub1 = subscriber_factory.get_subscription("Topic1").fuse();

        let topic1a = block_on(async {
            let mut result = Vec::new();

            loop {
                futures::select!(
                    item = sub1.select_next_some() => result.push(item),
                    default => break,
                );
            }
            result
        });

        assert_eq!(topic1a.len(), 4);
        assert_eq!(topic1a[0].a, 1);
        assert_eq!(topic1a[1].a, 3);
        assert_eq!(topic1a[2].a, 5);
        assert_eq!(topic1a[3].a, 7);

        let messages2 = vec![
            TopicPayload::new("Topic1", Dummy {
                a: 11u32,
                b: "one one".to_string(),
            }),
            TopicPayload::new("Topic2", Dummy {
                a: 22u32,
                b: "two two".to_string(),
            }),
            TopicPayload::new("Topic1", Dummy {
                a: 33u32,
                b: "three three".to_string(),
            }),
        ];

        block_on(async move {
            stream::iter(messages2).map(Ok).forward(publisher).await.unwrap();
        });

        let topic1b = block_on(async { sub1.collect::<Vec<Dummy>>().await });

        assert_eq!(topic1b.len(), 2);
        assert_eq!(topic1b[0].a, 11);
        assert_eq!(topic1b[1].a, 33);

        let sub2 = subscriber_factory.get_subscription("Topic2");

        let topic2 = block_on(async { sub2.collect::<Vec<Dummy>>().await });

        assert_eq!(topic2.len(), 4);
        assert_eq!(topic2[0].a, 2);
        assert_eq!(topic2[1].a, 4);
        assert_eq!(topic2[2].a, 6);
        assert_eq!(topic2[3].a, 22);
    }
}
