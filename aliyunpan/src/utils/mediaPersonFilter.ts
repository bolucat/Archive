import type { MediaLibraryItem } from '../types/media'

export function matchesMediaPerson(item: MediaLibraryItem, name: string, personId?: number): boolean {
  const credits = [item.credits, ...item.seasons?.map(season => season.credits) || [], ...item.collectionMovies?.map(movie => movie.credits) || []]
  const people = [
    ...credits.flatMap(credit => [...credit?.cast || [], ...credit?.crew || []]),
    ...item.seasons?.flatMap(season => season.episodes?.flatMap(episode => episode.crew || []) || []) || []
  ]
  const normalized = name.trim().toLocaleLowerCase()
  return people.some(person => personId && person.id ? person.id === personId : !!normalized && person.name.trim().toLocaleLowerCase() === normalized)
}
