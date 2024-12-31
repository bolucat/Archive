﻿using NLog;
using System;
using System.IO;
using System.IO.Compression;
using System.Text;

namespace Shadowsocks.Controller
{
    public static class FileManager
    {
        private static Logger logger = LogManager.GetCurrentClassLogger();

        public static bool ByteArrayToFile(string fileName, byte[] content)
        {
            try
            {
                using (var fs = new FileStream(fileName, FileMode.Create, FileAccess.Write))
                    fs.Write(content, 0, content.Length);
                return true;
            }
            catch (Exception ex)
            {
                logger.Error(ex);
            }
            return false;
        }

        public static void UncompressFile(string fileName, byte[] content)
        {
            // Because the uncompressed size of the file is unknown,
            // we are using an arbitrary buffer size.
            byte[] buffer = new byte[4096];
            int n;

            using(var fs = File.Create(fileName))
            using (var input = new GZipStream(new MemoryStream(content),
                    CompressionMode.Decompress, false))
            {
                while ((n = input.Read(buffer, 0, buffer.Length)) > 0)
                {
                    fs.Write(buffer, 0, n);
                }
            }
        }

        public static string NonExclusiveReadAllText(string path)
        {
            return NonExclusiveReadAllText(path, Encoding.Default);
        }

        public static string NonExclusiveReadAllText(string path, Encoding encoding)
        {
            try
            {
                using (var fs = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
                using (var sr = new StreamReader(fs, encoding))
                {
                    return sr.ReadToEnd();
                }
            }
            catch (Exception ex)
            {
                logger.Error(ex);
                throw ex;
            }
        }
    }
}
