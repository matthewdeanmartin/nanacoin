import { User } from '../api/models';

/** Commerce and offers name members by number; everything else by "user-N". */
export function memberNumber(user: Pick<User, 'id'> | null | undefined): number {
  return user ? Number(user.id.replace('user-', '')) : NaN;
}

export function userIdOf(member: number): string {
  return `user-${member}`;
}

export function nameOf(household: readonly User[], member: number): string {
  return household.find((u) => u.id === userIdOf(member))?.display_name ?? `Member ${member}`;
}
