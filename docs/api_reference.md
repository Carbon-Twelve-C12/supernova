# SuperNova Blockchain API Reference

This document details the APIs available in the SuperNova blockchain implementation. SuperNova provides both RESTful APIs and JSON-RPC APIs for interacting with the blockchain, including querying data, sending transactions, and managing the node.

## API Types Overview

SuperNova provides two different types of APIs:

1. **RESTful API** - Modern, resource-oriented API using standard HTTP methods and JSON responses
2. **JSON-RPC API** - Bitcoin-compatible API following the JSON-RPC 2.0 specification

## RESTful API

The RESTful API provides a comprehensive interface for interacting with the SuperNova blockchain using standard HTTP methods and JSON responses. This API is designed to be easy to use and follows RESTful principles.

### API Documentation

For complete RESTful API documentation, please refer to the following resources:

- [Blockchain API](./api/blockchain.md) - Access blocks and transactions
- [Wallet API](./api/wallet.md) - Manage wallets, addresses, and transactions
- [Admin API](./api/admin.md) - Node administration and management
- [Statistics API](./api/statistics.md) - Blockchain analytics and metrics

### Default Endpoint

RESTful API is available at: `http://localhost:8080/api/v1/`

### Authentication

Authentication for the RESTful API is performed using API keys. Include your API key in the request headers:

```
Authorization: Bearer YOUR_API_KEY
```

## JSON-RPC API

SuperNova provides a JSON-RPC 2.0 compatible API that can be accessed via HTTP or WebSocket connections. All API requests should be sent to the configured RPC endpoint.

**Default Endpoints:**
- HTTP: `http://localhost:8332`
- WebSocket: `ws://localhost:8332/ws`

### Authentication

By default, API access requires authentication to prevent unauthorized access. Two authentication methods are supported:

1. **HTTP Basic Authentication**: Username and password can be configured in the node configuration file.
2. **API Key**: An API key can be provided in the request headers.

Example HTTP header with API key:
```
X-API-Key: your-api-key-here
```

### General Structure

#### Request Format

All requests should follow the JSON-RPC 2.0 specification, with the following structure:

```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "method": "method-name",
  "params": {}
}
```

Where:
- `jsonrpc`: Always "2.0" to indicate JSON-RPC 2.0 protocol
- `id`: A unique identifier for the request, which will be included in the response
- `method`: The name of the API method to call
- `params`: An object containing the parameters for the method call

#### Response Format

Responses follow the JSON-RPC 2.0 specification:

```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "result": {}
}
```

Or in case of an error:

```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "error": {
    "code": -32000,
    "message": "Error message"
  }
}
```

## Blockchain Methods

### `getblockchaininfo`

Returns information about the blockchain state.

**Parameters**: None

**Result**:
```json
{
  "chain": "main",
  "blocks": 1234,
  "headers": 1234,
  "bestblockhash": "00000000000000000007ab12ca7931bed88ddb0e36edc99b063c6d469d6375b4",
  "difficulty": 123456789,
  "mediantime": 1574121600,
  "verificationprogress": 0.9999,
  "pruned": false,
  "pruneheight": 0,
  "chainwork": "0000000000000000000000000000000000000000000000000000000000000000"
}
```

### `getblock`

Returns information about a block with the given hash.

**Parameters**:
1. `blockhash` (string, required): The hash of the block
2. `verbosity` (number, optional, default=1): 0 for hex encoded data, 1 for a JSON object, and 2 for a JSON object with transaction data

**Result** (verbosity = 1):
```json
{
  "hash": "00000000000000000007ab12ca7931bed88ddb0e36edc99b063c6d469d6375b4",
  "confirmations": 1,
  "size": 1234,
  "height": 1234,
  "version": 1,
  "merkleroot": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
  "tx": [
    "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
  ],
  "time": 1574121600,
  "nonce": 2083236893,
  "bits": "1d00ffff",
  "difficulty": 123456789,
  "previousblockhash": "00000000000000000007ab12ca7931bed88ddb0e36edc99b063c6d469d6375b4"
}
```

### `getblockhash`

Returns the hash of the block at the specified height.

**Parameters**:
1. `height` (number, required): The height of the block

**Result**:
```
"00000000000000000007ab12ca7931bed88ddb0e36edc99b063c6d469d6375b4"
```

### `getbestblockhash`

Returns the hash of the best (tip) block in the longest blockchain.

**Parameters**: None

**Result**:
```
"00000000000000000007ab12ca7931bed88ddb0e36edc99b063c6d469d6375b4"
```

### `getblockcount`

Returns the number of blocks in the longest blockchain.

**Parameters**: None

**Result**:
```
1234
```

### `getdifficulty`

Returns the proof-of-work difficulty as a multiple of the minimum difficulty.

**Parameters**: None

**Result**:
```
123456789
```

## Transaction Methods

### `gettransaction`

Returns detailed information about a transaction.

**Parameters**:
1. `txid` (string, required): The transaction ID
2. `include_watchonly` (boolean, optional, default=true): Include watch-only addresses in balance calculations

**Result**:
```json
{
  "txid": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
  "hash": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
  "version": 1,
  "size": 225,
  "vsize": 225,
  "weight": 900,
  "locktime": 0,
  "vin": [
    {
      "txid": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
      "vout": 0,
      "scriptSig": {
        "asm": "3045022100eb9faea9299f701450fde413047d68e25c4b03e33a9e5391ee210d0a8a7bc59a02200aeb9bda89a1b27dd34e0186bd2869d4c84e43c65a0d20a1d951a3cd7bc35094[ALL] 03be5e0b5184e2a9db06bc60612775da16a6c5279dd5354d8ddd8e943c0187324",
        "hex": "483045022100eb9faea9299f701450fde413047d68e25c4b03e33a9e5391ee210d0a8a7bc59a02200aeb9bda89a1b27dd34e0186bd2869d4c84e43c65a0d20a1d951a3cd7bc3509401210303be5e0b5184e2a9db06bc60612775da16a6c5279dd5354d8ddd8e943c0187324"
      },
      "sequence": 4294967295
    }
  ],
  "vout": [
    {
      "value": 50.0,
      "n": 0,
      "scriptPubKey": {
        "asm": "OP_DUP OP_HASH160 7f9b1a7fb68d60c536c2fd8aeaa53a8f3cc025a8 OP_EQUALVERIFY OP_CHECKSIG",
        "hex": "76a9147f9b1a7fb68d60c536c2fd8aeaa53a8f3cc025a888ac",
        "reqSigs": 1,
        "type": "pubkeyhash",
        "addresses": [
          "1CK6KHY6MHgYvmRQ4PAafKYDrg1ejbH1cE"
        ]
      }
    }
  ],
  "blockhash": "00000000000000000007ab12ca7931bed88ddb0e36edc99b063c6d469d6375b4",
  "confirmations": 1,
  "time": 1574121600,
  "blocktime": 1574121600
}
```

### `getrawtransaction`

Returns the raw transaction data.

**Parameters**:
1. `txid` (string, required): The transaction ID
2. `verbose` (boolean, optional, default=false): If false, return a string, otherwise return a JSON object

**Result** (verbose=false):
```
"01000000010000000000000000000000000000000000000000000000000000000000000000ffffffff4d04ffff001d0104455468652054696d65732030332f4a616e2f32303039204368616e63656c6c6f72206f6e206272696e6b206f66207365636f6e64206261696c6f757420666f722062616e6b73ffffffff0100f2052a01000000434104678afdb0fe5548271967f1a67130b7105cd6a828e03909a67962e0ea1f61deb649f6bc3f4cef38c4f35504e51ec112de5c384df7ba0b8d578a4c702b6bf11d5fac00000000"
```

**Result** (verbose=true): Same as `gettransaction`

### `sendrawtransaction`

Submits a raw transaction to the network.

**Parameters**:
1. `hexstring` (string, required): The hex string of the raw transaction
2. `maxfeerate` (number, optional, default=0.1): Reject transactions whose fee rate is higher than the specified value (in NOVA/kB)

**Result**:
```
"4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
```

## Mempool Methods

### `getmempoolinfo`

Returns details on the active state of the transaction memory pool.

**Parameters**: None

**Result**:
```json
{
  "size": 10,
  "bytes": 2500,
  "usage": 3000,
  "maxmempool": 300000000,
  "mempoolminfee": 0.00001000,
  "minrelaytxfee": 0.00001000
}
```

### `getrawmempool`

Returns all transaction IDs in the memory pool.

**Parameters**:
1. `verbose` (boolean, optional, default=false): True for a JSON object, false for an array of transaction IDs

**Result** (verbose=false):
```json
[
  "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
  "a08e6907dbbd3d809776dbfc5d82e371b764ed838b5655e72f463568df1aadf0"
]
```

**Result** (verbose=true):
```json
{
  "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b": {
    "size": 225,
    "fee": 0.00050000,
    "time": 1574121600,
    "height": 1234,
    "descendantcount": 1,
    "descendantsize": 225,
    "descendantfees": 50000,
    "ancestorcount": 1,
    "ancestorsize": 225,
    "ancestorfees": 50000
  }
}
```

## Network Methods

### `getnetworkinfo`

Returns information about the node's connection to the network.

**Parameters**: None

**Result**:
```json
{
  "version": 1000000,
  "subversion": "/SuperNova:1.0.0/",
  "protocolversion": 70015,
  "localservices": "000000000000000d",
  "localrelay": true,
  "timeoffset": 0,
  "connections": 8,
  "networks": [
    {
      "name": "ipv4",
      "limited": false,
      "reachable": true,
      "proxy": "",
      "proxy_randomize_credentials": false
    },
    {
      "name": "ipv6",
      "limited": false,
      "reachable": true,
      "proxy": "",
      "proxy_randomize_credentials": false
    }
  ],
  "relayfee": 0.00001000,
  "incrementalfee": 0.00001000,
  "localaddresses": [
    {
      "address": "192.168.1.100",
      "port": 8333,
      "score": 1
    }
  ],
  "warnings": ""
}
```

### `getpeerinfo`

Returns data about each connected network node.

**Parameters**: None

**Result**:
```json
[
  {
    "id": 1,
    "addr": "192.168.1.101:8333",
    "addrlocal": "192.168.1.100:8333",
    "services": "000000000000000d",
    "lastsend": 1574121600,
    "lastrecv": 1574121600,
    "bytessent": 12345,
    "bytesrecv": 54321,
    "conntime": 1574120000,
    "pingtime": 0.01,
    "minping": 0.01,
    "version": 70015,
    "subver": "/SuperNova:1.0.0/",
    "inbound": false,
    "startingheight": 1234,
    "banscore": 0,
    "synced_headers": 1234,
    "synced_blocks": 1234
  }
]
```

### `addnode`

Attempts to add or remove a node from the addnode list, or to try a connection to a node once.

**Parameters**:
1. `node` (string, required): The node (IP address:port)
2. `command` (string, required): 'add' to add a node to the list, 'remove' to remove a node from the list, 'onetry' to try a connection to the node once

**Result**: None

## Mining Methods

### `getmininginfo`

Returns a JSON object containing mining-related information.

**Parameters**: None

**Result**:
```json
{
  "blocks": 1234,
  "currentblockweight": 4000,
  "currentblocktx": 10,
  "difficulty": 123456789,
  "networkhashps": 1000000000000,
  "pooledtx": 10,
  "chain": "main",
  "warnings": ""
}
```

### `getblocktemplate`

Returns data needed to construct a block.

**Parameters**:
1. `template_request` (object, optional): A JSON object with template request parameters
```json
{
  "mode": "template",
  "capabilities": ["longpoll", "coinbasetxn", "coinbasevalue", "proposal", "serverlist", "workid"]
}
```

**Result**:
```json
{
  "version": 1,
  "previousblockhash": "00000000000000000007ab12ca7931bed88ddb0e36edc99b063c6d469d6375b4",
  "transactions": [
    {
      "data": "01000000013ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa4b1e5e4a0000000000ffffffff0100f2052a010000001976a914f7c6c1f9f6142107ed293c8fbf3f94110ff2eb0888ac00000000",
      "txid": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
      "hash": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
      "depends": [],
      "fee": 1000,
      "sigops": 4,
      "weight": 900
    }
  ],
  "coinbaseaux": {
    "flags": ""
  },
  "coinbasevalue": 5000000000,
  "longpollid": "00000000000000000007ab12ca7931bed88ddb0e36edc99b063c6d469d6375b4 1234",
  "target": "0000000000000000000000000000000000000000000000000000000000000000",
  "mintime": 1574121600,
  "mutable": [
    "time",
    "transactions",
    "prevblock"
  ],
  "noncerange": "00000000ffffffff",
  "sigoplimit": 80000,
  "sizelimit": 4000000,
  "curtime": 1574121600,
  "bits": "1d00ffff",
  "height": 1235
}
```

### `submitblock`

Attempts to submit a new block to the network.

**Parameters**:
1. `hexdata` (string, required): The hex-encoded block data to submit
2. `dummy` (string, optional, default=""): Dummy value, for compatibility with other RPC implementations

**Result**: None if successful, error message if failed

## Wallet Methods

### `getbalance`

Returns the total available balance in the wallet.

**Parameters**:
1. `dummy` (string, optional): Remains for backward compatibility
2. `minconf` (number, optional, default=0): Only include transactions confirmed at least this many times
3. `include_watchonly` (boolean, optional, default=true): Include balance in watch-only addresses

**Result**:
```
100.00000000
```

### `listunspent`

Returns an array of unspent transaction outputs in the wallet.

**Parameters**:
1. `minconf` (number, optional, default=1): The minimum confirmations to filter
2. `maxconf` (number, optional, default=9999999): The maximum confirmations to filter
3. `addresses` (array, optional): A JSON array of addresses to filter
```json
[
  "1CK6KHY6MHgYvmRQ4PAafKYDrg1ejbH1cE"
]
```

**Result**:
```json
[
  {
    "txid": "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b",
    "vout": 0,
    "address": "1CK6KHY6MHgYvmRQ4PAafKYDrg1ejbH1cE",
    "scriptPubKey": "76a9147f9b1a7fb68d60c536c2fd8aeaa53a8f3cc025a888ac",
    "amount": 50.0,
    "confirmations": 1,
    "spendable": true,
    "solvable": true
  }
]
```

### `sendfrom`

Send an amount to a given address.

**Parameters**:
1. `fromaccount` (string, required): The name of the account to send funds from
2. `toaddress` (string, required): The address to send funds to
3. `amount` (number, required): The amount to send (in NOVA)
4. `minconf` (number, optional, default=1): Only use funds with at least this many confirmations
5. `comment` (string, optional): A comment used to store what the transaction is for
6. `comment_to` (string, optional): A comment to store the name of the person or organization to which you're sending the transaction

**Result**:
```
"4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
```

### `sendtoaddress`

Send an amount to a given address.

**Parameters**:
1. `address` (string, required): The address to send to
2. `amount` (number, required): The amount to send (in NOVA)
3. `comment` (string, optional): A comment used to store what the transaction is for
4. `comment_to`