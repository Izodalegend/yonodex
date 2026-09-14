const { createLibp2p } = require('libp2p')
const { tcp } = require('@libp2p/tcp')
const { plaintext } = require('@libp2p/plaintext')
const { mplex } = require('@libp2p/mplex')

async function startNode() {
  const node = await createLibp2p({
    addresses: {
      listen: ['/ip4/0.0.0.0/tcp/9000']
    },
    transports: [tcp()],
    connectionEncryptors: [plaintext()],
    streamMuxers: [mplex()],
  })

  await node.start()
  console.log('✅ P2P node started')
  console.log('Peer ID:', node.peerId.toString())
  console.log('Listening on:', node.getMultiaddrs().map(a => a.toString()))
  
  node.addEventListener('peer:connect', (evt) => {
    console.log('🔗 Peer connected:', evt.detail.toString())
  })
  
  return node
}

startNode()
