const { createLibp2p } = require('libp2p')
const { tcp } = require('@libp2p/tcp')
const { plaintext } = require('@libp2p/plaintext')
const { mplex } = require('@libp2p/mplex')
const { gossipsub } = require('@libp2p/gossipsub')

// In-memory order storage
const orders = []

async function startRelay() {
  const node = await createLibp2p({
    addresses: {
      listen: ['/ip4/0.0.0.0/tcp/9000']
    },
    transports: [tcp()],
    connectionEncryptors: [plaintext()],
    streamMuxers: [mplex()],
    pubsub: gossipsub({
      allowPublishToZeroPeers: true,
      emitSelf: true
    })
  })

  await node.start()
  console.log('✅ P2P Order Relay started')
  console.log('Peer ID:', node.peerId.toString())
  console.log('Listening on:', node.getMultiaddrs().map(a => a.toString()))

  // Subscribe to order topic
  const topic = 'njalla-orders'
  await node.pubsub.subscribe(topic)
  console.log(`📡 Subscribed to topic: ${topic}`)

  // Handle incoming orders
  node.pubsub.addEventListener('message', (evt) => {
    if (evt.detail.topic !== topic) return
    try {
      const order = JSON.parse(evt.detail.data.toString())
      console.log('📦 Order received:', order)
      orders.push(order)
    } catch (e) {
      console.error('Invalid order:', e.message)
    }
  })

  // Publish a test order every 30 seconds
  setInterval(() => {
    const testOrder = {
      id: Date.now(),
      pair: 'ETH/USDC',
      side: 'buy',
      price: (1800 + Math.random() * 50).toFixed(2),
      amount: (0.1 + Math.random() * 0.5).toFixed(2),
      timestamp: new Date().toISOString()
    }
    const data = Buffer.from(JSON.stringify(testOrder))
    node.pubsub.publish(topic, data)
    console.log('📤 Published test order:', testOrder)
  }, 30000)

  // Show orders every minute
  setInterval(() => {
    console.log(`📊 Total orders: ${orders.length}`)
  }, 60000)

  return node
}

startRelay().catch(console.error)
