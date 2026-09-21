// Yonodex Desktop Client - Tor-enabled libp2p relay
// Whitepaper Layer 4, Section 6.1: Tor Onion Transport
//
// CLI arguments:
//   --listen-port <port>   Local port to bind (default 4001)
//   --dial <multiaddr>     Peer multiaddr to dial on startup
//   --no-onion             Don't read .onion or advertise (dialer-only mode)

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

const TOPIC_ORDERS = 'yonodex-orders'
const orders = []

// ---- CLI parsing ----
function parseArgs(argv) {
  const args = {
    listenPort: 4001,
    dial: null,
    noOnion: false,
  }
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--listen-port') {
      args.listenPort = Number(argv[++i])
    } else if (a === '--dial') {
      args.dial = argv[++i]
    } else if (a === '--no-onion') {
      args.noOnion = true
    }
  }
  return args
}

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
  const args = parseArgs(process.argv)
  console.log(`⚙️  Config: port=${args.listenPort} dial=${args.dial ?? 'none'} noOnion=${args.noOnion}`)

  let onionAddress = null
  let announceMultiaddr = null

  if (!args.noOnion) {
    onionAddress = await readOwnOnionAddress()
    // libp2p multiaddr format expects the bare base32 pubkey (no .onion suffix)
    const onionKey = onionAddress.replace(/\.onion$/, '')
    announceMultiaddr = `/onion3/${onionKey}:${args.listenPort}`
    console.log(`🔑 Our .onion address: ${onionAddress}`)
  } else {
    console.log('🔇 Dialer-only mode: no hidden service advertised')
  }

  const listenMultiaddr = `/ip4/127.0.0.1/tcp/${args.listenPort}`

  const pubsub = gossipsub({
    allowPublishToZeroPeers: true,
    emitSelf: true,
  })

  const nodeConfig = {
    addresses: {
      listen: [listenMultiaddr],
    },
    transports: [
      torTransport({
        socksHost: '127.0.0.1',
        socksPort: 9050,
      }),
    ],
    connectionEncrypters: [noise()],
    streamMuxers: [mplex()],
    // Hidden service rendezvous needs 30-90s. Default is 10s.
    connectionManager: {
      dialTimeout: 180_000,
    },
    services: {
      pubsub,
      identify: identify(),
    },
  }

  if (announceMultiaddr) {
    nodeConfig.addresses.announce = [announceMultiaddr]
  }

  const node = await createLibp2p(nodeConfig)

  await node.start()

  console.log('✅ Tor-enabled P2P relay started')
  console.log('   Peer ID:', node.peerId.toString())
  console.log('   Listening on:', node.getMultiaddrs().map((a) => a.toString()))
  if (announceMultiaddr) {
    console.log(`   Advertised as: ${announceMultiaddr}`)
  }

  await node.services.pubsub.subscribe(TOPIC_ORDERS)
  console.log(`📡 Subscribed to topic: ${TOPIC_ORDERS}`)

  // Dial peer if provided
  if (args.dial) {
    try {
      const dialMa = multiaddr(args.dial)
      console.log(`🎯 Dialing peer: ${dialMa.toString()}`)
      const conn = await node.dial(dialMa)
      console.log(`🔗 Connected to peer: ${conn.remotePeer.toString()}`)
    } catch (err) {
      console.error(`❌ Dial failed: ${err.message}`)
    }
  }

  // Log peer connections
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

  // Publish a test order every 30s
  setInterval(() => {
    const testOrder = {
      id: Date.now(),
      peerId: node.peerId.toString(),
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

  setInterval(() => {
    const peers = node.getPeers().map((p) => p.toString())
    console.log(`📊 Total orders: ${orders.length} | Peers: ${peers.length} [${peers.join(', ')}]`)
  }, 60_000)

  return node
}

startTorRelay().catch((err) => {
  console.error('Fatal:', err)
  process.exit(1)
})