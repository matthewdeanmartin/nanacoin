import { Listing } from '../api/models';

export const GOOD_DEED = 'good_deed';

/** A good deed Nana rewards with newly issued coins. Rewards are in whole NC. */
export interface GoodDeedIdea { title: string; description: string; reward: number }

/**
 * The starter set behind "Add 25 good deeds". SMBC's Nana issues new money for
 * good deeds; these are the household-sized kind. Titles are unique so adding
 * the set twice never duplicates a deed.
 */
export const GOOD_DEED_IDEAS: readonly GoodDeedIdea[] = [
  { title: 'Help a sibling with homework', description: 'Sit down together until it makes sense.', reward: 5 },
  { title: 'Call a grandparent just to chat', description: 'No asking for anything. Just catching up.', reward: 3 },
  { title: 'Write a thank-you note', description: 'On paper, to someone who helped you.', reward: 3 },
  { title: 'Carry groceries for a neighbor', description: 'Ask first, then carry them all the way in.', reward: 4 },
  { title: 'Clean up a park or playground', description: 'Bring a bag and gloves; fill the bag.', reward: 8 },
  { title: 'Donate outgrown clothes or toys', description: 'Sort them, wash them, and drop them off.', reward: 6 },
  { title: 'Read a bedtime story to someone younger', description: 'Voices for every character.', reward: 2 },
  { title: 'Make breakfast for the family', description: 'And clean up after, which is the real deed.', reward: 6 },
  { title: 'Water a neighbor’s plants while they are away', description: 'Every day they are gone.', reward: 5 },
  { title: 'Teach someone a skill you know', description: 'Knots, card tricks, long division: anything.', reward: 5 },
  { title: 'Apologize and make it right', description: 'Say what you did, then fix what you can.', reward: 4 },
  { title: 'Let someone else pick the movie', description: 'And watch it without complaining.', reward: 2 },
  { title: 'Walk a neighbor’s dog', description: 'A proper walk, with a bag.', reward: 4 },
  { title: 'Shovel or sweep a neighbor’s walk', description: 'Snow, leaves, or whatever the season brings.', reward: 6 },
  { title: 'Bake something to share', description: 'Lemon bars are traditional.', reward: 5 },
  { title: 'Visit someone who is lonely', description: 'An hour of company counts.', reward: 6 },
  { title: 'Fix something broken instead of replacing it', description: 'Glue, tape, thread or a screwdriver.', reward: 4 },
  { title: 'Help set up or clean up a family event', description: 'Arrive early or stay late.', reward: 5 },
  { title: 'Pick up litter on a walk', description: 'At least ten pieces.', reward: 2 },
  { title: 'Make a birthday card by hand', description: 'Drawings strongly encouraged.', reward: 3 },
  { title: 'Share your snack', description: 'An even split, and the bigger half goes to them.', reward: 1 },
  { title: 'Stand up for someone being teased', description: 'Kindly and out loud.', reward: 8 },
  { title: 'Help cook a family dinner', description: 'Chopping, stirring and tasting all count.', reward: 5 },
  { title: 'Tidy a shared space you did not mess up', description: 'Without being asked.', reward: 3 },
  { title: 'Plant something', description: 'A tree, a flower, or herbs on the windowsill.', reward: 4 },
];

export function isGoodDeed(listing: Pick<Listing, 'kind'>): boolean {
  return listing.kind === GOOD_DEED;
}

/** Ideas not already posted as an active good deed, matched by title. */
export function missingIdeas(active: readonly Pick<Listing, 'title' | 'kind' | 'status'>[]): GoodDeedIdea[] {
  const posted = new Set(active.filter((l) => isGoodDeed(l) && l.status === 'ACTIVE').map((l) => l.title.toLocaleLowerCase()));
  return GOOD_DEED_IDEAS.filter((idea) => !posted.has(idea.title.toLocaleLowerCase()));
}
