export default class Config {
  // 网络请求配置
  static referer = 'https://www.aliyundrive.com/drive'
  static downAgent = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/89.0.4577.63 Safari/537.36'
  static userAgent = 'Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) aDrive/4.12.0 Chrome/108.0.5359.215 Electron/22.3.24 Safari/537.36'
  static loginUrl = 'https://auth.aliyundrive.com/v2/oauth/authorize?login_type=custom&response_type=code&redirect_uri=https%3A%2F%2Fwww.aliyundrive.com%2Fsign%2Fcallback&client_id=25dzX3vbYqktVxyX&state=%7B%22origin%22%3A%22https%3A%2F%2Fwww.aliyundrive.com%2F%22%7D'
  static loginUrlAccount = 'https://passport.aliyundrive.com/mini_login.htm?lang=zh_cn&appName=aliyun_drive&appEntrance=web&styleType=auto&bizParams=&notLoadSsoView=false&notKeepLogin=false&isMobile=false&&rnd=0.1100330129139'
  // macOS 代码签名和公证配置
  static APPLE_ID = ''
  static APPLE_PASSWORD = ''
  static APPLE_TEAM_ID = ''

  // Supabase 认证
  static SUPABASE_URL = 'https://ltqipofjjqjlbbfsgihi.supabase.co'
  static SUPABASE_ANON_KEY = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Imx0cWlwb2ZqanFqbGJiZnNnaWhpIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NDA0OTQxNjgsImV4cCI6MjA1NjA3MDE2OH0.g1vk-DaWbicHnSVZoGqskd0vOu-NuWtsDaMvFhe22mE'

  // TMDB 配置
  static TMDB_API_KEY = ''

  // 用户认证后端
  static AUTH_API_BASE = 'http://localhost:3000'

  static CREEM_API_KEY = ''   // 测试环境
  static CREEM_PRODUCT_ID = 'prod_4DB3OByC3DoLNN9pe54QqS'

}
