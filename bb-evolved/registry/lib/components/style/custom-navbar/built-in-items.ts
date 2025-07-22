import { CustomNavbarItemInit } from './custom-navbar-item'
import { messages } from './messages/messages'
import { ranking } from './ranking/ranking'
import { userInfo } from './user-info/user-info'
import { logo } from './logo/logo'
import { home } from './home/home'
import { games } from './games/games'
import { blanks } from './flexible-blank/flexible-blank'
import { bangumi, music, shop, creations, lives } from './simple-links/simple-links'
import { match } from './match/match'
import { upload } from './upload/upload'
import { search } from './search/search'
import { feeds } from './feeds/feeds'
import { subscriptions } from './subscriptions/subscriptions'
import { watchlater } from './watchlater/watchlater'
import { favorites } from './favorites/favorites'
import { history } from './history/history'
import { manga } from './manga/manga'

const [blank1, blank2, blank3, blank4] = blanks
export const getBuiltInItems = (): CustomNavbarItemInit[] => [
  blank1,
  logo,
  home,
  bangumi,
  ranking,
  music,
  games,
  lives,
  shop,
  match,
  manga,
  blank2,
  search,
  blank3,
  userInfo,
  messages,
  feeds,
  subscriptions,
  watchlater,
  favorites,
  history,
  creations,
  upload,
  blank4,
]
