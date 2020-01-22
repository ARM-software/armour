
cd ~/Downloads
aws ec2 run-instances --image-id ami-07263d8579fa30a5e --count 17 --instance-type t2.large --key-name some-key --security-groups arm-default --region eu-west-2
sleep 120s
aws ec2 describe-instances --filters "Name=image-id,Values=ami-07263d8579fa30a5e"  --region eu-west-2 --query 'Reservations[*].Instances[*].NetworkInterfaces[*].PrivateIpAddresses[*].[Association.PublicIp]' --output text > ips
