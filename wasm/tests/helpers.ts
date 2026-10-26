import { Client } from '../pkg/node';

// Tests default to testnet: the SDK refuses to connect to an endpoint whose schema does
// not match the one it was compiled against, and testnet leads mainnet through upgrades.
// Override with BULLET_API_ENDPOINT.
export const TEST_ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.testnet.bullet.xyz';

export async function connectReadOnlyClient() {
  return connectForUserActions([]);
}

export async function connectForUserActions(actions: string[]) {
  return Client.builder()
    .network(TEST_ENDPOINT)
    .userActions(actions)
    .build();
}
