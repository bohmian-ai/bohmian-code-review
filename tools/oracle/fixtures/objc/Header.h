#import <Foundation/Foundation.h>

@interface HeaderOnly : NSObject
@property (nonatomic, readonly) NSInteger count;
- (void)increment;
@end

@protocol HeaderProtocol
- (void)requiredMethod;
@end
