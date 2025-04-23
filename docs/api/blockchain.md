# Blockchain API

The Blockchain API provides access to blockchain data, including blocks, transactions, and blockchain status information.

## Endpoints

### Get Blockchain Information

```
GET /api/v1/blockchain/info
```

Returns general information about the state of the blockchain.

**Response:**

```json
{
  "chain": "main",
  "blocks": 750000,
  "headers": 750000,
  "bestblockhash": "000000000000000000034bdb3c01621a0c84787efafd807aa104c9a327537a39",
  "difficulty": 29337160522857.9,
  "mediantime": 1658180961,
  "verificationprogress": 0.999999,
  "pruned": false,
  "softforks": {
    "taproot": {
      "status": "active",
      "height": 709632
    }
  },
  "size_on_disk": 486932398499
}
```

### Get Block by Height

```
GET /api/v1/blockchain/block/height/{height}
```

Returns block data for the specified block height.

**Parameters:**

- `height` (path parameter): Block height

**Optional Query Parameters:**

- `verbosity` (query parameter): Detail level (0-2, default: 1)

**Response:**

```json
{
  "hash": "000000000000000000034bdb3c01621a0c84787efafd807aa104c9a327537a39",
  "confirmations": 10,
  "height": 750000,
  "version": 536870912,
  "versionHex": "20000000",
  "merkleroot": "2ff4c48573f76f347afcfc3e6705b3f090daac4b3eef5a69535022e0c6ba08c6",
  "time": 1658180961,
  "mediantime": 1658177650,
  "nonce": 1025613359,
  "bits": "17136ae5",
  "difficulty": 29337160522857.9,
  "previousblockhash": "00000000000000000007bab82abcba59006f2a86fe34a5d9a35fc366c6e5c1e3",
  "nextblockhash": "00000000000000000005d4e7a728e88ac203035b4f7480957f8b1a5a18b75895",
  "tx": [
    "b0930e3ce1e28a08b7f3a492a356da5a12d1c848ffb0352a7ee4798ce27fd3c0",
    // ...more transactions...
  ],
  "size": 1561042,
  "weight": 3992775,
  "strippedsize": 855729
}
```

### Get Block by Hash

```
GET /api/v1/blockchain/block/hash/{hash}
```

Returns block data for the specified block hash.

**Parameters:**

- `hash` (path parameter): Block hash

**Optional Query Parameters:**

- `verbosity` (query parameter): Detail level (0-2, default: 1)

**Response:**

Same format as Get Block by Height.

### Get Transaction

```
GET /api/v1/blockchain/transaction/{txid}
```

Returns detailed information about a specific transaction.

**Parameters:**

- `txid` (path parameter): Transaction ID (hash)

**Response:**

```json
{
  "txid": "b0930e3ce1e28a08b7f3a492a356da5a12d1c848ffb0352a7ee4798ce27fd3c0",
  "hash": "b0930e3ce1e28a08b7f3a492a356da5a12d1c848ffb0352a7ee4798ce27fd3c0",
  "version": 1,
  "size": 289,
  "vsize": 289,
  "weight": 1156,
  "locktime": 0,
  "vin": [
    {
      "txid": "5a2f08a488c0f06359ec0d5a8b3210ef3af7cab0c55b598cb7db14a4abc6ffca",
      "vout": 0,
      "scriptSig": {
        "asm": "304502210089e97dffbe0bd241fbabbec5089c147724ab356eb6c2de650b2e94fd4b8a8e7102202a46b237b42ef575ece9387cba40091b5eb5c90f4607f9c3b9c2aea7851391d4[ALL] 03a5309088d757c654569733742825ae6bfb26d88a373d914293e8a80c9da6a19d",
        "hex": "48304502210089e97dffbe0bd241fbabbec5089c147724ab356eb6c2de650b2e94fd4b8a8e7102202a46b237b42ef575ece9387cba40091b5eb5c90f4607f9c3b9c2aea7851391d4012103a5309088d757c654569733742825ae6bfb26d88a373d914293e8a80c9da6a19d"
      },
      "sequence": 4294967295
    }
  ],
  "vout": [
    {
      "value": 0.01234567,
      "n": 0,
      "scriptPubKey": {
        "asm": "OP_DUP OP_HASH160 b42d66434c64e4598f0d1fa7320a1cad8f49f8ea OP_EQUALVERIFY OP_CHECKSIG",
        "hex": "76a914b42d66434c64e4598f0d1fa7320a1cad8f49f8ea88ac",
        "reqSigs": 1,
        "type": "pubkeyhash",
        "addresses": [
          "1HcXd9WZxVt5yBXdGVcCFkHFbzGP8qQXUD"
        ]
      }
    }
  ],
  "hex": "01000000...",
  "blockhash": "000000000000000000034bdb3c01621a0c84787efafd807aa104c9a327537a39",
  "confirmations": 10,
  "time": 1658180961,
  "blocktime": 1658180961
}
```

### Submit Transaction

```
POST /api/v1/blockchain/transaction
```

Submits a raw transaction to the network.

**Request Body:**

```json
{
  "raw_tx": "01000000017b1eabe0209b1fe794124575ef807057c77ada2138ae4fa8d6c4de0398a14f3f00000000494830450221008949f0cb400094ad2b5eb399d59d01c14d73d8fe6e96df1a7150deb388ab8935022079656090d7f6bac4c9a94e0aad311a4268e082a725f8aeae0573fb12ff866a5f01ffffffff01f0ca052a010000001976a914cbc20a7664f2f69e5355aa427045bc15e7c6c77288ac00000000"
}
```

**Response:**

```json
{
  "txid": "7d85e07f990e9af06c336f2a091bfdc741a4f656e95ada1d792c9c56694e5ac0"
}
```

## Error Responses

All endpoints may return the following errors:

- `400 Bad Request`: Invalid parameters
- `404 Not Found`: Resource not found
- `500 Internal Server Error`: Server error

Example error response:

```json
{
  "error": "Block not found",
  "code": 404
}
``` 