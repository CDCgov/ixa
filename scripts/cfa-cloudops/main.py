from cfa.cloudops import CloudClient

def main():
    # initialize
    print("Initializing CloudClient...")
    cc = CloudClient(dotenv_path="env", use_sp = True)
#    print("Hello from ixa-cfa-cloudops!")
#    files = cc.list_blob_files("input-test")
#    print(f"Files in 'input-test' container: {files}")
    container_name = cc.package_and_upload_dockerfile(
        registry_name = "my_azure_registry",
        repo_name = "ixa-bench",
        tag = "latest"
    )

    cc.create_pool(
        "getting-started-pool",
        mounts = [('input-test', 'inputs')],
        container_image_name = container_name,
        vm_size = "standard_d8s_v3",
        max_autoscale_nodes = 5
    )

    cc.create_job(
        "getting-started-job",
        pool_name = "getting-started-pool",
        exist_ok = True
    )

    cc.add_task(
        job_name = "getting-started-job",
        command_line = "python3 /inputs/main.py --user Ryan"
    )

    cc.monitor_job(
        "getting-started-job"
    )

    # delete the job
    cc.delete_job("getting-started-job")

    # delete the pool
    cc.delete_pool("getting-started-pool")

if __name__ == "__main__":
    main()
