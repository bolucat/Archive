//
//  QRCodeUtils.m
//  ShadowsocksX-NG
//
//  Created by 邱宇舟 on 16/6/8.
//  Copyright © 2016年 qiuyuzhou. All rights reserved.
//

#import <Foundation/Foundation.h>
#import <CoreImage/CoreImage.h>
#import <AppKit/AppKit.h>

void ScanQRCodeOnScreen(void) {    
    /* check system version and permission status */
    if (@available(macOS 10.12, *)) {
        BOOL hasPermission = CGPreflightScreenCaptureAccess();
        NSLog(@"Screen Recording Permission Status: %@", hasPermission ? @"Granted" : @"Not Granted");
        
        if (!hasPermission) {
            NSLog(@"Requesting Screen Recording Permission...");
            CGRequestScreenCaptureAccess();
            
            /* check permission status after request */
            hasPermission = CGPreflightScreenCaptureAccess();
            NSLog(@"Screen Recording Permission Status After Request: %@", hasPermission ? @"Granted" : @"Not Granted");
            
            if (!hasPermission) {
                NSLog(@"Screen Recording Permission Denied");
                
                /* send notification about permission missing */
                [[NSNotificationCenter defaultCenter]
                 postNotificationName:@"NOTIFY_FOUND_SS_URL"
                 object:nil
                 userInfo:@{
                     @"urls": @[],
                     @"source": @"qrcode",
                     @"error": @"Screen Recording permission required. Please grant permission in System Preferences and restart ShadowsocksX-NG"
                 }];
                
                /* open system privacy settings */
                [[NSWorkspace sharedWorkspace] openURL:[NSURL URLWithString:@"x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"]];
                return;
            }
        }
        
        NSLog(@"Proceeding with screen capture...");
    }
    
    /* displays[] Quartz display ID's */
    CGDirectDisplayID   *displays = nil;
    CGDisplayCount      dspCount = 0;
    
    /* variables for collecting scan information */
    NSMutableDictionary *scanInfo = [NSMutableDictionary dictionary];
    NSMutableArray *foundSSUrls = [NSMutableArray array];
    NSMutableArray *foundQRCodes = [NSMutableArray array];
    
    /* How many active displays do we have? */
    CGError err = CGGetActiveDisplayList(0, NULL, &dspCount);
    
    if(err != CGDisplayNoErr) {
        [[NSNotificationCenter defaultCenter]
         postNotificationName:@"NOTIFY_FOUND_SS_URL"
         object:nil
         userInfo:@{
             @"urls": @[],
             @"source": @"qrcode",
             @"error": @"Failed to get display list"
         }];
        return;
    }
    
    scanInfo[@"displayCount"] = @(dspCount);
    NSLog(@"Found %d displays", dspCount);
    
    /* Allocate enough memory to hold all the display IDs we have. */
    displays = calloc((size_t)dspCount, sizeof(CGDirectDisplayID));
    
    // Get the list of active displays
    err = CGGetActiveDisplayList(dspCount, displays, &dspCount);
    
    if(err != CGDisplayNoErr) {
        free(displays);
        [[NSNotificationCenter defaultCenter]
         postNotificationName:@"NOTIFY_FOUND_SS_URL"
         object:nil
         userInfo:@{
             @"urls": @[],
             @"source": @"qrcode",
             @"error": @"Failed to get display information"
         }];
        return;
    }
    
    CIDetector *detector = [CIDetector detectorOfType:@"CIDetectorTypeQRCode"
                                            context:nil
                                            options:@{ CIDetectorAccuracy:CIDetectorAccuracyHigh }];
    
    int totalQRCodesFound = 0;
    int validSSUrlsFound = 0;
    
    for (unsigned int displaysIndex = 0; displaysIndex < dspCount; displaysIndex++) {
        CGImageRef image = CGDisplayCreateImage(displays[displaysIndex]);
        NSArray *features = [detector featuresInImage:[CIImage imageWithCGImage:image]];
        
        /* count total QR codes found */
        totalQRCodesFound += (int)features.count;
        
        for (CIQRCodeFeature *feature in features) {
            NSLog(@"Found QR Code: %@", feature.messageString);
            [foundQRCodes addObject:feature.messageString];
            
            if ([feature.messageString hasPrefix:@"ss://"]) {
                NSURL *url = [NSURL URLWithString:feature.messageString];
                if (url) {
                    [foundSSUrls addObject:url];
                    validSSUrlsFound++;
                }
            }
        }
        CGImageRelease(image);
    }
    
    free(displays);
    
    /* prepare notification information */
    NSString *notificationTitle;
    NSString *notificationSubtitle;
    NSString *notificationBody;
    
    if (totalQRCodesFound == 0) {
        notificationTitle = [NSString stringWithFormat:@"Scanned %d displays", dspCount];
        notificationSubtitle = @"No QR codes found";
        notificationBody = @"Try adjusting the QR code position on your screen";
    } else if (validSSUrlsFound == 0) {
        notificationTitle = [NSString stringWithFormat:@"Found %d QR code(s)", totalQRCodesFound];
        notificationSubtitle = @"No valid Shadowsocks URLs";
        notificationBody = @"QR codes found are not Shadowsocks configuration";
    } else {
        notificationTitle = [NSString stringWithFormat:@"Found %d Shadowsocks URL(s)", validSSUrlsFound];
        notificationSubtitle = [NSString stringWithFormat:@"Scanned %d displays, found %d QR codes", dspCount, totalQRCodesFound];
        notificationBody = @"Processing Shadowsocks configuration...";
    }
    
    [[NSNotificationCenter defaultCenter]
     postNotificationName:@"NOTIFY_FOUND_SS_URL"
     object:nil
     userInfo:@{
         @"urls": foundSSUrls,
         @"source": @"qrcode",
         @"title": notificationTitle,
         @"subtitle": notificationSubtitle,
         @"body": notificationBody,
         @"scanInfo": @{
             @"displayCount": @(dspCount),
             @"totalQRCodes": @(totalQRCodesFound),
             @"validURLs": @(validSSUrlsFound)
         }
     }];
}

NSImage* createQRImage(NSString *string, NSSize size) {
    NSImage *outputImage = [[NSImage alloc]initWithSize:size];
    [outputImage lockFocus];
    
    // Setup the QR filter with our string
    CIFilter *filter = [CIFilter filterWithName:@"CIQRCodeGenerator"];
    [filter setDefaults];
    
    NSData *data = [string dataUsingEncoding:NSUTF8StringEncoding];
    [filter setValue:data forKey:@"inputMessage"];
    /*
     L: 7%
     M: 15%
     Q: 25%
     H: 30%
     */
    [filter setValue:@"Q" forKey:@"inputCorrectionLevel"];
    
    CIImage *image = [filter valueForKey:@"outputImage"];
    
    // Calculate the size of the generated image and the scale for the desired image size
    CGRect extent = CGRectIntegral(image.extent);
    CGFloat scale = MIN(size.width / CGRectGetWidth(extent), size.height / CGRectGetHeight(extent));
    
    CGImageRef bitmapImage = [NSGraphicsContext.currentContext.CIContext createCGImage:image fromRect:extent];
    
    CGContextRef graphicsContext = NSGraphicsContext.currentContext.CGContext;
    
    CGContextSetInterpolationQuality(graphicsContext, kCGInterpolationNone);
    CGContextScaleCTM(graphicsContext, scale, scale);
    CGContextDrawImage(graphicsContext, extent, bitmapImage);
    
    // Cleanup
    CGImageRelease(bitmapImage);
    
    [outputImage unlockFocus];
    return outputImage;
}
