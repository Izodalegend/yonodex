// Yonodex Desktop Client - Tor-enabled libp2p relay
// Whitepaper Layer 4, Section 6.1: Tor Onion Transport
// Whitepaper Layer 5, Section 7.1: Order book lives in Rust - Node.js is transport only
//
// CLI arguments:
//   --listen-port <port>   Local port to bind (default 4001)
//   --dial <multiaddr>     Peer multiaddr to dial on startup
//   --no-onion             Don't read .onion or advertise (dialer-only mode)
//
// IPC (Rust <-> Node.js):
//   stdin  (Rust -> Node): {"type":"publish_order", "order": {...}}
//                          {"type":"publish_cancel", "order_id":"...","vector_clock":{},"remote_peer":"..."}
//                          {"type":"publish_trade", "trade": {...}}
//   stdout (Node -> Rust): {"type":"order_received", "order": {...}}
//                          {"type":"cancel_received", "order_id":"...","vector_clock":{},"remote_peer":"..."}
//                          {"type":"trade_received", "trade": {...}}
//                          {"type":"status", "ready":true, "peer_count":N}

import { createLibp2p } from 'libp2p'
import { noise } from '@chainsafe/libp2p-noise'
import { mplex } from '@libp2p/mplex'
import { gossipsub } from '@libp2p/gossipsub'
import { identify } from '@libp2p/identify'
import { ping } from '@libp2p/ping'
import { multiaddr } from '@multiformats/multiaddr'
import { torTransport } from './p2p-tor-transport.mjs'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

// ---- Redirect console.log/error to stderr so stdout is IPC-only ----
const _logWrite = (...args) =>
  process.stderr.write(args.map((a) => String(a)).join(' ') + '\n')
console.log = _logWrite
console.error = _logWrite

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
const TOPIC_TRADES = 'yonodex-trades'

// ---- IPC helpers ----
function ipcWrite(msg) {
  process.stdout.write(JSON.stringify(msg) + '\n')
}

function startIpcReader(onPublishOrder, onPublishCancel, onPublishTrade) {
  let buffer = ''
  process.stdin.setEncoding('utf8')
  process.stdin.on('data', (chunk) => {
    buffer += chunk
    let idx
    while ((idx = buffer.indexOf('\n')) !== -1) {
      const line = buffer.slice(0, idx).trim()
      buffer = buffer.slice(idx + 1)
      if (!line) continue
      try {
        const msg = JSON.parse(line)
        if (msg.type === 'publish_order') {
          onPublishOrder(msg.order)
        } else if (msg.type === 'publish_cancel') {
          onPublishCancel(msg.order_id, msg.vector_clock, msg.remote_peer)
        } else if (msg.type === 'publish_trade') {
          onPublishTrade(msg.trade)
        }
      } catch (err) {
        console.error('[ipc] bad message:', err.message)
      }
    }
  })
  process.stdin.on('end', () => {
    console.log('[ipc] stdin closed - shutting down')
    process.exit(0)
  })
}

// ---- CLI parsing ----
function parseArgs(argv) {
  const args = { listenPort: 4001, dial: null, noOnion: false }
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i]
    if (a === '--listen-port') args.listenPort = Number(argv[++i])
    else if (a === '--dial') args.dial = argv[++i]
    else if (a === '--no-onion') args.noOnion = true
  }
  return args
}

async function readOwnOnionAddress() {
  try {
    const raw = await readFile(ONION_HOSTNAME_PATH, 'utf8')
    return raw.trim()
  } catch (err) {
    throw new Error(
      `Could not read .onion hostname from ${ONION_HOSTNAME_PATH}. (${err.message})`
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
    addresses: { listen: [listenMultiaddr] },
    transports: [
      torTransport({ socksHost: '127.0.0.1', socksPort: 9050 }),
    ],
    connectionEncrypters: [noise()],
    streamMuxers: [mplex()],
    connectionManager: { dialTimeout: 180_000, minConnections: 1 },
    services: {
      pubsub,
      identify: identify(),
      ping: ping(),
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
  if (announceMultiaddr) console.log(`   Advertised as: ${announceMultiaddr}`)

  await node.services.pubsub.subscribe(TOPIC_ORDERS)
  await node.services.pubsub.subscribe(TOPIC_TRADES)
  console.log(`📡 Subscribed to topics: ${TOPIC_ORDERS}, ${TOPIC_TRADES}`)

  // ---- Wire Rust <-> gossipsub ----
  startIpcReader(
    (order) => {
      const data = Buffer.from(JSON.stringify({ kind: 'order', order }))
      node.services.pubsub.publish(TOPIC_ORDERS, data)
      console.log(`[ipc] published order ${order.id}`)
    },
    (orderId, vectorClock, remotePeer) => {
      const data = Buffer.from(
        JSON.stringify({ kind: 'cancel', order_id: orderId, vector_clock: vectorClock, remote_peer: remotePeer })
      )
      node.services.pubsub.publish(TOPIC_ORDERS, data)
      console.log(`[ipc] published cancel for ${orderId}`)
    },
    (trade) => {
      const data = Buffer.from(JSON.stringify({ kind: 'trade', trade }))
      node.services.pubsub.publish(TOPIC_TRADES, data)
      console.log(`[ipc] published trade ${trade.id}`)
    }
  )

  ipcWrite({ type: 'status', ready: true, peer_count: 0 })

  // ---- Handle incoming messages ----
  node.services.pubsub.addEventListener('message', (evt) => {
    const topic = evt.detail.topic
    try {
      const msg = JSON.parse(evt.detail.data.toString())

      if (topic === TOPIC_ORDERS) {
        if (msg.kind === 'order') {
          console.log('📦 order_received:', msg.order.id)
          ipcWrite({ type: 'order_received', order: msg.order })
        } else if (msg.kind === 'cancel') {
          console.log('📦 cancel_received:', msg.order_id)
          ipcWrite({
            type: 'cancel_received',
            order_id: msg.order_id,
            vector_clock: msg.vector_clock,
            remote_peer: msg.remote_peer,
          })
        }
      } else if (topic === TOPIC_TRADES) {
        if (msg.kind === 'trade') {
          console.log('📦 trade_received:', msg.trade.id)
          ipcWrite({ type: 'trade_received', trade: msg.trade })
        }
      }
    } catch (e) {
      console.error('Invalid message:', e.message)
    }
  })

  // ---- Dial with retry ----
  async function dialWithRetry(dialMa, label = 'dial') {
    const maxAttempts = 5
    const baseBackoffMs = 5_000
    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      const startedAt = Date.now()
      try {
        console.log(`🎯 ${label} attempt ${attempt}/${maxAttempts}`)
        const conn = await node.dial(dialMa)
        console.log(`🔗 Connected to peer in ${Date.now() - startedAt}ms`)
        return true
      } catch (err) {
        console.error(`❌ ${label} attempt ${attempt} failed: ${err.message}`)
        if (attempt === maxAttempts) return false
        const waitMs = Math.min(baseBackoffMs * Math.pow(2, attempt - 1), 60_000)
        await new Promise((r) => setTimeout(r, waitMs))
      }
    }
    return false
  }

  // ---- Auto-reconnect ----
  if (args.dial) {
    const dialMa = multiaddr(args.dial)
    await dialWithRetry(dialMa, 'initial-dial')

    let reconnecting = false
    node.addEventListener('peer:disconnect', () => {
      if (reconnecting) return
      reconnecting = true
      setTimeout(async () => {
        console.log(`♻️  Reconnecting`)
        try {
          await dialWithRetry(dialMa, 'reconnect')
        } finally {
          reconnecting = false
        }
      }, 10_000)
    })
  }

  node.addEventListener('peer:connect', () => {
    console.log(`🔗 Peer connected`)
  })

  // ---- Status heartbeat ----
  setInterval(() => {
    const peers = node.getPeers().map((p) => p.toString())
    ipcWrite({ type: 'status', ready: true, peer_count: peers.length })
  }, 15_000)

  return node
}

startTorRelay().catch((err) => {
  console.error('Fatal:', err)
  process.exit(1)
})