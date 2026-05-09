import { Client } from '../pkg/node';

export const TEST_ENDPOINT =
  process.env.BULLET_API_ENDPOINT ?? 'https://tradingapi.bullet.xyz';

export async function connectReadOnlyClient() {
  return connectForUserActions([]);
}

export async function connectForUserActions(actions: string[]) {
  return Client.builder()
    .network(TEST_ENDPOINT)
    .userActions(actions)
    .build();
}
