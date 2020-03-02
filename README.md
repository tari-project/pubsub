# Tari Pubsub

Single publisher with multiple subscribers to topic messages.

Pubsub is an abstraction over `tari_broadcast_channel`

## Example

```
    // Create a new channel with a buffer of 10 messages
    let (mut publisher, subscriber_factory) = pubsub_channel(10);

    // Create a struct that we want to use as messages
    #[derive(Debug, Clone)]
    struct Dummy {
        a: u32,
        b: String,
    }

    let messages = vec![
        TopicPayload::new("Topic1", Dummy { a: 1u32, b: "one".to_string() }),
        TopicPayload::new("Topic2", Dummy { a: 2u32, b: "two".to_string() }),
        TopicPayload::new("Topic1", Dummy { a: 3u32, b: "three".to_string() }),
        TopicPayload::new("Topic2", Dummy { a: 4u32, b: "four".to_string() }),
        TopicPayload::new("Topic1", Dummy { a: 5u32, b: "five".to_string() }),
        TopicPayload::new("Topic2", Dummy { a: 6u32, b: "size".to_string() }),
        TopicPayload::new("Topic1", Dummy { a: 7u32, b: "seven".to_string() }),
    ];

    // PubSub is generic over the message type, so it's very simple to publish messages of type `Dummy`
    block_on(async {
        for m in messages {
            publisher.send(m).await.unwrap();
        }
    });

    // Subscribers can subscribe to specific topics; and receive messages in the form of an async stream
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
```
