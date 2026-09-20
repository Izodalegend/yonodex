// Yonodex Desktop Client - Tor-enabled libp2p relay
// Whitepaper Layer 4, Section 6.1: Tor Onion Transport
// Sub-task 5: route peer connections over SOCKS5 to Tor
//
// This node:
//   - Listens on 127.0.0.1:4001 (Tor hidden service forwards .onion traffic here)
//   - Dials peers via /onion3/... multiaddrs through Tor SOCKS5 (127.0.0.1:9050)
//   - Advertises its own .onion address so peers can reach it
//   - Uses gossipsub to relay orders between peers

import { createLibp2p } from 'libp2p'
import { noise } from '@chainsafe/libp2p-noise'
import { mplex } from '@libp2p/mplex'
import { gossipsub } from '@libp2p/gossipsub'
import { identify } from '@libp2p/identify'
import { multiaddr } from '@multiformats/multiaddr'
import { torTransport } from './p2p-tor-transport.mjs'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const __dirname = dirname(fileURLToPath(import.meta.url))
const ONION_HOSTNAME_PATH = join(
  __dirname,
  'src-tauri',
  'resources',
  'tor',
  'data',
  'onion-service',
  'hostname'
)

const LISTEN_PORT = 4001
const TOPIC_ORDERS = 'yonodex-orders'

// In-memory order storage (dev only - Phase 2 will use CRDT)
const orders = []

async function readOwnOnionAddress() {
  try {
    const raw = await readFile(ONION_HOSTNAME_PATH, 'utf8')
    return raw.trim()
  } catch (err) {
    throw new Error(
      `Could not read .onion hostname from ${ONION_HOSTNAME_PATH}. ` +
      `Is Tor running and has it created the hidden service yet? (${err.message})`
    )
  }
}

async function startTorRelay() {
  const onionAddress = await readOwnOnionAddress()
  console.log(`🔑 Our .onion address: ${onionAddress}`)

  // Build the multiaddrs we want to advertise.
  // The listener binds locally; Tor maps external .onion traffic to it.
  const listenMultiaddr = `/ip4/127.0.0.1/tcp/${LISTEN_PORT}`
  const announceMultiaddr = `/onion3/${onionAddress}:${LISTEN_PORT}`

  const pubsub = gossipsub({
    allowPublishToZeroPeers: true,
    emitSelf: true,
  })

  const node = await createLibp2p({
    addresses: {
      listen: [listenMultiaddr],
      announce: [announceMultiaddr],
    },
    transports: [
      torTransport({
        socksHost: '127.0.0.1',
        socksPort: 9050,
      }),
    ],
    connectionEncryptors: [noise()],
    streamMuxers: [mplex()],
    services: {
      pubsub,
      identify: identify(),
    },
  })

  await node.start()

  console.log('✅ Tor-enabled P2P relay started')
  console.log('   Peer ID:', node.peerId.toString())
  console.log('   Listening on:', node.getMultiaddrs().map((a) => a.toString()))
  console.log(`   Advertised as: ${announceMultiaddr}`)

  await node.services.pubsub.subscribe(TOPIC_ORDERS)
  console.log(`📡 Subscribed to topic: ${TOPIC_ORDERS}`)

  // Log incoming peer connections
  node.addEventListener('peer:connect', (evt) => {
    console.log(`🔗 Peer connected: ${evt.detail.toString()}`)
  })
  node.addEventListener('peer:disconnect', (evt) => {
    console.log(`🔌 Peer disconnected: ${evt.detail.toString()}`)
  })

  // Handle incoming orders
  node.services.pubsub.addEventListener('message', (evt) => {
    if (evt.detail.topic !== TOPIC_ORDERS) return
    try {
      const order = JSON.parse(evt.detail.data.toString())
      console.log('📦 Order received:', order)
      orders.push(order)
    } catch (e) {
      console.error('Invalid order:', e.message)
    }
  })

  // Publish a test order every 30s (dev only)
  setInterval(() => {
    const testOrder = {
      id: Date.now(),
      pair: 'ETH/USDC',
      side: 'buy',
      price: (1800 + Math.random() * 50).toFixed(2),
      amount: (0.1 + Math.random() * 0.5).toFixed(2),
      timestamp: new Date().toISOString(),
    }
    const data = Buffer.from(JSON.stringify(testOrder))
    node.services.pubsub.publish(TOPIC_ORDERS, data)
    console.log('📤 Published test order:', testOrder)
  }, 30_000)

  // Stats every minute
  setInterval(() => {
    console.log(`📊 Total orders: ${orders.length}`)
  }, 60_000)

  return node
}

startTorRelay().catch((err) => {
  console.error('Fatal:', err)
  process.exit(1)
})