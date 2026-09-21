// Yonodex Desktop Client - libp2p transport over Tor SOCKS5
// Whitepaper Layer 4, Section 6.1: Tor Onion Transport
//
// Wraps @libp2p/tcp's factory transport so that:
//   - Dials to /onion3/... multiaddrs go through Tor's SOCKS5 proxy
//   - Plain TCP dials (for local testing) still work
//   - Listener stays plain TCP: Tor forwards .onion traffic to 127.0.0.1:4001
//     via the hidden service configuration in torrc-yonodex

import { SocksClient } from 'socks'
import { tcp } from '@libp2p/tcp'
import { CustomProgressEvent } from 'progress-events'

const isOnion3 = (ma) => ma.toString().startsWith('/onion3/')

export function torTransport(options = {}) {
  const socksHost = options.socksHost ?? '127.0.0.1'
  const socksPort = options.socksPort ?? 9050
  const onionDialTimeout = options.onionDialTimeout ?? 180_000

  return (components) => {
    // Get the base TCP transport instance from the factory
    const base = tcp(options)(components)
    const log = components.logger.forComponent('libp2p:tor')

    // Save original methods before overriding
    const originalConnect = base._connect.bind(base)
    const originalDialFilter = base.dialFilter.bind(base)

    // Override _connect to route onion3 dials through SOCKS5
    base._connect = async (ma, dialOptions) => {
      if (!isOnion3(ma)) {
        // Non-onion address: use default TCP behavior (for local testing)
        return originalConnect(ma, dialOptions)
      }

      dialOptions.signal.throwIfAborted()
      dialOptions.onProgress?.(new CustomProgressEvent('tor:open-connection'))

      // Parse /onion3/<52-char-base32>:<port>
            const comps = ma.getComponents()
      const onionComp = comps.find((c) => c.name === 'onion3')
      if (!onionComp) {
        throw new Error(`invalid onion3 multiaddr: ${ma}`)
      }
      // The onion3 component value is "<base32-pubkey>:<port>" - the port is
      // encoded inside the component itself, not as a separate /tcp/ component.
      const rawValue = String(onionComp.value)
      const [onionKey, embeddedPort] = rawValue.split(':')
      const port = Number(embeddedPort) || 4001
      const host = `${onionKey}.onion`

      log('dialing %s:%d via SOCKS5 %s:%d', host, port, socksHost, socksPort)

      let socket
      try {
        const result = await SocksClient.createConnection({
          proxy: {
            host: socksHost,
            port: socksPort,
            type: 5,
          },
          command: 'connect',
          destination: { host, port },
          timeout: onionDialTimeout,
        })
        socket = result.socket
      } catch (err) {
        log.error('SOCKS5 dial to %s:%d failed - %e', host, port, err)
        throw new Error(`Tor dial failed to ${host}:${port}: ${err.message}`)
      }

      // Apply socket options matching @libp2p/tcp behavior
      if (dialOptions.noDelay !== false) socket.setNoDelay(true)
      if (dialOptions.keepAlive !== false) socket.setKeepAlive(true)
      if (dialOptions.allowHalfOpen === false) socket.allowHalfOpen = false

      return socket
    }

    // Override dialFilter to accept /onion3/... in addition to TCP addresses
    base.dialFilter = (multiaddrs) => {
      return multiaddrs.filter((ma) => {
        if (isOnion3(ma)) return true
        return originalDialFilter([ma]).length > 0
      })
    }

    return base
  }
}